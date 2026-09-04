//! Persist canonical blocks under `--data-dir` via RocksDB.

use anyhow::{Context, Result, bail};
use ivory_chain::BlockStore;
use ivory_core::Block;
use ivory_primitives::H256;
use ivory_storage::RocksDbBackend;
use std::path::Path;

const KEY_HEAD: &[u8] = b"head";

/// On-disk canonical chain (blocks by hash, height index, head).
pub struct ChainPersist {
    db: RocksDbBackend,
    archive: bool,
    archive_keep: u64,
}

impl ChainPersist {
    /// Open (or create) `{data-dir}/chain` (archive all bodies).
    ///
    /// # Errors
    ///
    /// RocksDB open failures.
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with(path, true, 256)
    }

    /// Open with archive policy.
    ///
    /// # Errors
    ///
    /// RocksDB open failures.
    pub fn open_with(path: &Path, archive: bool, archive_keep: u64) -> Result<Self> {
        std::fs::create_dir_all(path).with_context(|| format!("mkdir {}", path.display()))?;
        let db = RocksDbBackend::open(path).context("open chain rocksdb")?;
        Ok(Self {
            db,
            archive,
            archive_keep: archive_keep.max(1),
        })
    }

    /// Whether all snapshots should be retained.
    #[must_use]
    pub const fn is_archive(&self) -> bool {
        self.archive
    }

    /// In-memory snapshot window when not archiving.
    #[must_use]
    pub const fn archive_keep(&self) -> u64 {
        self.archive_keep
    }

    /// Insert genesis into `store` from disk, or return `None` if the DB is empty.
    ///
    /// When a chain is present, `expected_genesis` must hash-equal the stored
    /// genesis (`genesis.json` vs last `ivory run`).
    ///
    /// # Errors
    ///
    /// IO, codec, genesis mismatch, or missing keys.
    pub fn load_into(&self, store: &BlockStore, expected_genesis: &Block) -> Result<Option<u64>> {
        let Some(head_raw) = self.db.get(KEY_HEAD).context("read head")? else {
            return Ok(None);
        };
        let head_hash = H256::from_slice(&head_raw).context("head hash length")?;
        let head = self
            .get_block(&head_hash)?
            .context("head block missing from store")?;
        let genesis = self
            .get_block_by_height(0)?
            .context("canonical genesis missing")?;
        if genesis.hash() != expected_genesis.hash() {
            bail!(
                "persisted genesis {} does not match genesis.json {}",
                genesis.hash().to_hex(),
                expected_genesis.hash().to_hex()
            );
        }
        store
            .insert_genesis(genesis)
            .context("reload genesis into memory")?;
        for n in 1..=head.header.number {
            let block = self
                .get_block_by_height(n)?
                .with_context(|| format!("canonical block {n} missing"))?;
            store
                .insert_block(block)
                .with_context(|| format!("reload block {n}"))?;
        }
        Ok(Some(head.header.number))
    }

    /// Write `block` and refresh the canonical height index from `store`.
    ///
    /// # Errors
    ///
    /// Serialization or RocksDB write failures.
    pub fn persist_canonical(&self, store: &BlockStore, block: &Block) -> Result<()> {
        self.put_block(block)?;
        let Some(head) = store.head_block() else {
            return Ok(());
        };
        self.db
            .put(KEY_HEAD, head.hash().as_bytes())
            .context("write head")?;
        for n in 0..=head.header.number {
            let Some(b) = store.get_block_by_number(n) else {
                continue;
            };
            let keep =
                self.archive || n == 0 || head.header.number.saturating_sub(n) < self.archive_keep;
            if keep {
                self.put_block(&b)?;
            }
            self.put_height(n, b.hash())?;
        }
        if let Some(h) = store.head()
            && let Some(state) = store.state_at(&h)
        {
            let _ = self.persist_trie_nodes(&state.trie_nodes());
        }
        self.db.flush().context("flush chain")?;
        Ok(())
    }

    /// Persist Patricia account-trie nodes under `t` + hash.
    ///
    /// # Errors
    ///
    /// RocksDB write failures.
    pub fn persist_trie_nodes(&self, nodes: &[(ivory_primitives::H256, Vec<u8>)]) -> Result<()> {
        for (hash, bytes) in nodes {
            let mut key = Vec::with_capacity(33);
            key.push(b't');
            key.extend_from_slice(hash.as_bytes());
            self.db.put(&key, bytes).context("write trie node")?;
        }
        self.db.flush().context("flush trie nodes")?;
        Ok(())
    }

    /// Load a persisted trie node.
    ///
    /// # Errors
    ///
    /// RocksDB read failures.
    pub fn get_trie_node(&self, hash: &H256) -> Result<Option<Vec<u8>>> {
        let mut key = Vec::with_capacity(33);
        key.push(b't');
        key.extend_from_slice(hash.as_bytes());
        self.db.get(&key).context("read trie node")
    }

    fn put_block(&self, block: &Block) -> Result<()> {
        let bytes = bincode::serialize(block).context("encode block")?;
        self.db
            .put(&block_key(&block.hash()), &bytes)
            .context("write block")?;
        Ok(())
    }

    fn put_height(&self, number: u64, hash: H256) -> Result<()> {
        self.db
            .put(&height_key(number), hash.as_bytes())
            .with_context(|| format!("write height {number}"))?;
        Ok(())
    }

    fn get_block(&self, hash: &H256) -> Result<Option<Block>> {
        let Some(raw) = self.db.get(&block_key(hash)).context("read block")? else {
            return Ok(None);
        };
        let block = bincode::deserialize(&raw).context("decode block")?;
        Ok(Some(block))
    }

    fn get_block_by_height(&self, number: u64) -> Result<Option<Block>> {
        let Some(raw) = self
            .db
            .get(&height_key(number))
            .with_context(|| format!("read height {number}"))?
        else {
            return Ok(None);
        };
        let hash = H256::from_slice(&raw).context("height hash length")?;
        self.get_block(&hash)
    }
}

fn height_key(number: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(9);
    key.push(b'h');
    key.extend_from_slice(&number.to_be_bytes());
    key
}

fn block_key(hash: &H256) -> Vec<u8> {
    let mut key = Vec::with_capacity(33);
    key.push(b'b');
    key.extend_from_slice(hash.as_bytes());
    key
}

#[cfg(test)]
mod tests {
    use ivory_chain::BlockStore;
    use ivory_consensus::{ConsensusEngine, PoAConsensus};
    use ivory_core::{Block, BlockHeader, empty_list_roots};
    use ivory_crypto::keypair_from_byte;
    use ivory_primitives::{Bytes, H256, U256};

    use super::*;

    fn sealed_genesis(ts: u64) -> Block {
        let (sk, _, miner) = keypair_from_byte(1);
        let poa = PoAConsensus::from_secret(&sk).unwrap();
        let (tx_root, rx_root) = empty_list_roots();
        let mut header = BlockHeader {
            number: 0,
            parent_hash: H256::ZERO,
            timestamp: ts,
            miner,
            gas_limit: 30_000_000,
            gas_used: 0,
            state_root: H256::ZERO,
            transactions_root: tx_root,
            receipts_root: rx_root,
            difficulty: U256::ZERO,
            extra_data: Bytes::new(),
        };
        poa.seal_header(&mut header, &miner, &sk).unwrap();
        Block {
            header,
            transactions: Vec::new(),
            receipts: Vec::new(),
        }
    }

    fn poa() -> PoAConsensus {
        PoAConsensus::from_secret(&keypair_from_byte(1).0).unwrap()
    }

    #[test]
    fn empty_db_loads_none() {
        let dir = tempfile::tempdir().unwrap();
        let persist = ChainPersist::open(dir.path()).unwrap();
        let store = BlockStore::new(poa());
        let g = sealed_genesis(1);
        assert!(persist.load_into(&store, &g).unwrap().is_none());
    }

    #[test]
    fn persist_and_reload_genesis() {
        let dir = tempfile::tempdir().unwrap();
        let persist = ChainPersist::open(dir.path()).unwrap();
        let g = sealed_genesis(1);
        let store = BlockStore::new(poa());
        store.insert_genesis(g.clone()).unwrap();
        persist.persist_canonical(&store, &g).unwrap();
        drop(persist);

        let persist = ChainPersist::open(dir.path()).unwrap();
        let store2 = BlockStore::new(poa());
        let loaded = persist.load_into(&store2, &g).unwrap();
        assert_eq!(loaded, Some(0));
        assert_eq!(store2.head(), Some(g.hash()));
    }

    #[test]
    fn persist_writes_patricia_nodes() {
        use ivory_core::Account;
        use ivory_state::StateDB;

        let dir = tempfile::tempdir().unwrap();
        let persist = ChainPersist::open(dir.path()).unwrap();
        let g = sealed_genesis(1);
        let store = BlockStore::new(poa());
        store.insert_genesis(g.clone()).unwrap();
        let state = StateDB::new();
        let mut acc = Account::new();
        acc.balance = U256::from(7u64);
        state.set_account(keypair_from_byte(2).2, acc);
        store.record_state(g.hash(), state.fork());
        persist.persist_canonical(&store, &g).unwrap();
        let nodes = state.trie_nodes();
        assert!(!nodes.is_empty());
        for (hash, bytes) in &nodes {
            assert_eq!(
                persist.get_trie_node(hash).unwrap().as_deref(),
                Some(bytes.as_slice())
            );
        }
    }

    #[test]
    fn persist_reorg_reload_matches_replay() {
        use ivory_chain::{BlockProducer, ProduceParams, import_and_apply};
        use ivory_core::Account;
        use ivory_crypto::signed_transfer;
        use ivory_executor::Executor;
        use ivory_state::StateDB;
        use ivory_txpool::{TransactionPool, TxOrigin};

        let (sk, _, miner) = keypair_from_byte(9);
        let poa = PoAConsensus::from_secret(&sk).unwrap();
        let from = keypair_from_byte(1).2;
        let to_a = keypair_from_byte(2).2;
        let to_b = keypair_from_byte(3).2;

        let genesis_state = StateDB::new();
        let mut funded = Account::new();
        funded.balance = U256::from(1_000_000u64);
        genesis_state.set_account(from, funded);

        let (tx_root, rx_root) = empty_list_roots();
        let mut header = BlockHeader {
            number: 0,
            parent_hash: H256::ZERO,
            timestamp: 1,
            miner,
            gas_limit: 30_000_000,
            gas_used: 0,
            state_root: genesis_state.root_hash(),
            transactions_root: tx_root,
            receipts_root: rx_root,
            difficulty: U256::ZERO,
            extra_data: Bytes::new(),
        };
        poa.seal_header(&mut header, &miner, &sk).unwrap();
        let genesis = Block {
            header,
            transactions: Vec::new(),
            receipts: Vec::new(),
        };

        let store = BlockStore::new(poa.clone());
        store.insert_genesis(genesis.clone()).unwrap();
        store.record_state(genesis.hash(), genesis_state.fork());
        let live = genesis_state.fork();
        let pool = TransactionPool::new();

        let produce = |parent: &Block, parent_state: StateDB, to, ts| {
            let p = TransactionPool::new();
            p.add_transaction(
                signed_transfer(&keypair_from_byte(1).0, to, 0, U256::from(10u64), 21_000),
                TxOrigin::Local,
            )
            .unwrap();
            let exec = Executor::new(parent_state);
            BlockProducer::new()
                .produce_block(ProduceParams {
                    parent,
                    pool: &p,
                    executor: &exec,
                    consensus: &poa,
                    miner,
                    miner_key: &sk,
                    timestamp: ts,
                    max_txs: 8,
                })
                .unwrap()
        };

        let block_a = produce(&genesis, store.state_at(&genesis.hash()).unwrap(), to_a, 10);
        let block_b = produce(&genesis, store.state_at(&genesis.hash()).unwrap(), to_b, 11);
        import_and_apply(&store, &live, &pool, block_a).unwrap();
        import_and_apply(&store, &live, &pool, block_b.clone()).unwrap();

        let empty = TransactionPool::new();
        let exec = Executor::new(store.state_at(&block_b.hash()).unwrap());
        let block_b2 = BlockProducer::new()
            .produce_block(ProduceParams {
                parent: &block_b,
                pool: &empty,
                executor: &exec,
                consensus: &poa,
                miner,
                miner_key: &sk,
                timestamp: 12,
                max_txs: 8,
            })
            .unwrap();
        import_and_apply(&store, &live, &pool, block_b2.clone()).unwrap();

        let dir = tempfile::tempdir().unwrap();
        let persist = ChainPersist::open(dir.path()).unwrap();
        persist.persist_canonical(&store, &block_b2).unwrap();
        drop(persist);

        let persist = ChainPersist::open(dir.path()).unwrap();
        let store2 = BlockStore::new(poa);
        persist.load_into(&store2, &genesis).unwrap();
        let replay = genesis_state.fork();
        let exec = Executor::new(replay.clone());
        let head_n = store2.head_block().unwrap().header.number;
        for n in 1..=head_n {
            let block = store2.get_block_by_number(n).unwrap();
            let mut ctx =
                ivory_executor::ExecutionContext::new(block.header.number, block.header.timestamp);
            for tx in &block.transactions {
                exec.execute_transaction(tx, &mut ctx).unwrap();
            }
        }
        assert_eq!(
            replay.get_account(&to_b).map(|a| a.balance),
            live.get_account(&to_b).map(|a| a.balance)
        );
        assert_eq!(
            replay.get_account(&from).unwrap().nonce,
            live.get_account(&from).unwrap().nonce
        );
        assert!(
            replay
                .get_account(&to_a)
                .is_none_or(|a| a.balance.is_zero())
        );
    }

    #[test]
    fn load_refuses_bad_transactions_root() {
        use ivory_crypto::signed_transfer;

        let dir = tempfile::tempdir().unwrap();
        let persist = ChainPersist::open(dir.path()).unwrap();
        let g = sealed_genesis(1);
        let store = BlockStore::new(poa());
        store.insert_genesis(g.clone()).unwrap();
        persist.persist_canonical(&store, &g).unwrap();
        drop(persist);

        let db = ivory_storage::RocksDbBackend::open(dir.path()).unwrap();
        let mut key = Vec::from([b'b']);
        key.extend_from_slice(g.hash().as_bytes());
        let mut bad = g.clone();
        bad.transactions.push(signed_transfer(
            &keypair_from_byte(2).0,
            keypair_from_byte(3).2,
            0,
            U256::from(1u64),
            21_000,
        ));
        db.put(&key, &bincode::serialize(&bad).unwrap()).unwrap();
        drop(db);

        let persist = ChainPersist::open(dir.path()).unwrap();
        let store2 = BlockStore::new(poa());
        assert!(
            persist.load_into(&store2, &g).is_err(),
            "replay must re-validate list roots"
        );
    }

    #[test]
    fn genesis_mismatch_errors() {
        let dir = tempfile::tempdir().unwrap();
        let persist = ChainPersist::open(dir.path()).unwrap();
        let g = sealed_genesis(1);
        let store = BlockStore::new(poa());
        store.insert_genesis(g.clone()).unwrap();
        persist.persist_canonical(&store, &g).unwrap();
        drop(persist);

        let persist = ChainPersist::open(dir.path()).unwrap();
        let store2 = BlockStore::new(poa());
        let other = sealed_genesis(99);
        assert!(persist.load_into(&store2, &other).is_err());
    }
}
