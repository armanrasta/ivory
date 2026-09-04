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
}

impl ChainPersist {
    /// Open (or create) `{data-dir}/chain`.
    ///
    /// # Errors
    ///
    /// RocksDB open failures.
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path).with_context(|| format!("mkdir {}", path.display()))?;
        let db = RocksDbBackend::open(path).context("open chain rocksdb")?;
        Ok(Self { db })
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
            self.put_block(&b)?;
            self.put_height(n, b.hash())?;
        }
        self.db.flush().context("flush chain")?;
        Ok(())
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
    use ivory_core::{Block, BlockHeader};
    use ivory_crypto::keypair_from_byte;
    use ivory_primitives::{Bytes, H256, U256};

    use super::*;

    fn sealed_genesis(ts: u64) -> Block {
        let (sk, _, miner) = keypair_from_byte(1);
        let poa = PoAConsensus::from_secret(&sk).unwrap();
        let mut header = BlockHeader {
            number: 0,
            parent_hash: H256::ZERO,
            timestamp: ts,
            miner,
            gas_limit: 30_000_000,
            gas_used: 0,
            state_root: H256::ZERO,
            transactions_root: H256::ZERO,
            receipts_root: H256::ZERO,
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

        let mut header = BlockHeader {
            number: 0,
            parent_hash: H256::ZERO,
            timestamp: 1,
            miner,
            gas_limit: 30_000_000,
            gas_used: 0,
            state_root: genesis_state.root_hash(),
            transactions_root: H256::ZERO,
            receipts_root: H256::ZERO,
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
