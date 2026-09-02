//! In-memory block index and canonical head.

use std::collections::HashMap;

use ivory_consensus::{ConsensusEngine, PoAConsensus};
use ivory_core::{Block, Transaction};
use ivory_primitives::H256;
use ivory_state::StateDB;
use parking_lot::RwLock;

use crate::error::ChainError;

/// Where a transaction was included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TxLocation {
    /// Containing block hash.
    pub block_hash: H256,
    /// Block number.
    pub block_number: u64,
    /// Index in `block.transactions`.
    pub index: usize,
}

/// Indexed blocks plus optional per-height state snapshots.
pub struct BlockStore {
    by_hash: RwLock<HashMap<H256, Block>>,
    /// Canonical hash at each height (longest-chain view).
    by_number: RwLock<HashMap<u64, H256>>,
    head: RwLock<Option<H256>>,
    snapshots: RwLock<HashMap<u64, StateDB>>,
    tx_index: RwLock<HashMap<H256, TxLocation>>,
    consensus: PoAConsensus,
}

impl BlockStore {
    /// Empty store that validates headers with `consensus`.
    #[must_use]
    pub fn new(consensus: PoAConsensus) -> Self {
        Self {
            by_hash: RwLock::new(HashMap::new()),
            by_number: RwLock::new(HashMap::new()),
            head: RwLock::new(None),
            snapshots: RwLock::new(HashMap::new()),
            tx_index: RwLock::new(HashMap::new()),
            consensus,
        }
    }

    /// Hash of the canonical tip.
    #[must_use]
    pub fn head(&self) -> Option<H256> {
        *self.head.read()
    }

    /// Canonical tip block.
    #[must_use]
    pub fn head_block(&self) -> Option<Block> {
        let hash = self.head()?;
        self.get_block(&hash)
    }

    /// Look up a block by hash.
    #[must_use]
    pub fn get_block(&self, hash: &H256) -> Option<Block> {
        self.by_hash.read().get(hash).cloned()
    }

    /// Canonical block at `number`.
    #[must_use]
    pub fn get_block_by_number(&self, number: u64) -> Option<Block> {
        let hash = self.by_number.read().get(&number).copied()?;
        self.get_block(&hash)
    }

    /// Look up an included transaction by hash.
    #[must_use]
    pub fn get_transaction(&self, hash: &H256) -> Option<(Transaction, TxLocation)> {
        let loc = *self.tx_index.read().get(hash)?;
        let block = self.get_block(&loc.block_hash)?;
        let tx = block.transactions.get(loc.index)?.clone();
        Some((tx, loc))
    }

    /// Snapshot recorded at `number` (if the producer stored one).
    #[must_use]
    pub fn state_at_block(&self, number: u64) -> Option<StateDB> {
        self.snapshots.read().get(&number).cloned()
    }

    /// Record a cheap `Arc` clone of `state` at `number`.
    pub fn record_state(&self, number: u64, state: StateDB) {
        self.snapshots.write().insert(number, state);
    }

    /// Insert genesis (`number == 0`, `parent_hash == ZERO`).
    ///
    /// # Errors
    ///
    /// [`ChainError::InvalidGenesis`], consensus, or duplicate hash.
    pub fn insert_genesis(&self, block: Block) -> Result<H256, ChainError> {
        if block.header.number != 0 || block.header.parent_hash != H256::ZERO {
            return Err(ChainError::InvalidGenesis);
        }
        self.consensus.validate_header(&block.header, None)?;
        block.validate()?;
        self.insert_validated(block)
    }

    /// Insert a descendant of a known parent.
    ///
    /// Updates the canonical head when this block’s height is greater than the
    /// current head, or equal height with a lexicographically smaller hash.
    ///
    /// # Errors
    ///
    /// Unknown parent, wrong number, duplicate, consensus, or [`ivory_core::Block::validate`].
    pub fn insert_block(&self, block: Block) -> Result<H256, ChainError> {
        if block.header.number == 0 {
            return self.insert_genesis(block);
        }
        let parent_hash = block.header.parent_hash;
        let parent = self
            .get_block(&parent_hash)
            .ok_or(ChainError::UnknownParent)?;
        let expected = parent.header.number.saturating_add(1);
        if block.header.number != expected {
            return Err(ChainError::InvalidBlockNumber {
                expected,
                got: block.header.number,
            });
        }
        self.consensus
            .validate_header(&block.header, Some(&parent.header))?;
        block.validate()?;
        self.insert_validated(block)
    }

    fn insert_validated(&self, block: Block) -> Result<H256, ChainError> {
        let hash = block.hash();
        if self.by_hash.read().contains_key(&hash) {
            return Err(ChainError::DuplicateBlock);
        }
        let number = block.header.number;
        {
            let mut tx_index = self.tx_index.write();
            for (index, tx) in block.transactions.iter().enumerate() {
                tx_index.insert(
                    tx.hash(),
                    TxLocation {
                        block_hash: hash,
                        block_number: number,
                        index,
                    },
                );
            }
        }
        self.by_hash.write().insert(hash, block);
        self.maybe_update_canonical(hash, number);
        Ok(hash)
    }

    fn maybe_update_canonical(&self, hash: H256, number: u64) {
        let current = *self.head.read();
        let take = match current {
            None => true,
            Some(head_hash) => {
                let head_num = {
                    let by_hash = self.by_hash.read();
                    by_hash
                        .get(&head_hash)
                        .map(|b| b.header.number)
                        .unwrap_or(0)
                };
                number > head_num || (number == head_num && hash < head_hash)
            }
        };
        if !take {
            return;
        }
        *self.head.write() = Some(hash);
        self.rebuild_canonical(hash);
    }

    fn rebuild_canonical(&self, tip: H256) {
        let mut path = Vec::new();
        let mut cur = Some(tip);
        {
            let by_hash = self.by_hash.read();
            while let Some(h) = cur {
                let Some(block) = by_hash.get(&h) else {
                    break;
                };
                path.push((block.header.number, h));
                if block.header.number == 0 {
                    break;
                }
                cur = Some(block.header.parent_hash);
            }
        }
        let mut by_number = self.by_number.write();
        by_number.clear();
        for (n, h) in path {
            by_number.insert(n, h);
        }
    }
}

#[cfg(test)]
mod tests {
    use ivory_consensus::{ConsensusEngine, PoAConsensus, encode_seals};
    use ivory_core::{Block, BlockHeader};
    use ivory_crypto::keypair_from_byte;
    use ivory_primitives::{Address, Bytes, H256, SecretKey, U256};

    use super::*;

    fn miner_sk() -> SecretKey {
        keypair_from_byte(1).0
    }

    fn miner() -> Address {
        keypair_from_byte(1).2
    }

    fn poa() -> PoAConsensus {
        PoAConsensus::from_secret(&miner_sk()).unwrap()
    }

    fn sealed_header(number: u64, parent: H256, ts: u64) -> BlockHeader {
        let mut h = BlockHeader {
            number,
            parent_hash: parent,
            timestamp: ts,
            miner: miner(),
            gas_limit: 30_000_000,
            gas_used: 0,
            state_root: H256::ZERO,
            transactions_root: H256::ZERO,
            receipts_root: H256::ZERO,
            difficulty: U256::ZERO,
            extra_data: Bytes::new(),
        };
        poa().seal_header(&mut h, &miner(), &miner_sk()).unwrap();
        h
    }

    fn blk(number: u64, parent: H256, ts: u64) -> Block {
        Block {
            header: sealed_header(number, parent, ts),
            transactions: Vec::new(),
            receipts: Vec::new(),
        }
    }

    fn genesis() -> Block {
        blk(0, H256::ZERO, 1)
    }

    #[test]
    fn insert_genesis_sets_head() {
        let store = BlockStore::new(poa());
        let g = genesis();
        let hash = store.insert_genesis(g.clone()).unwrap();
        assert_eq!(store.head(), Some(hash));
        assert_eq!(store.head_block().unwrap().hash(), hash);
        assert_eq!(store.get_block_by_number(0).unwrap().hash(), hash);
        assert_eq!(store.get_block(&hash).unwrap(), g);
    }

    #[test]
    fn genesis_wrong_number_rejected() {
        let store = BlockStore::new(poa());
        let mut g = genesis();
        g.header.number = 1;
        poa()
            .seal_header(&mut g.header, &miner(), &miner_sk())
            .unwrap();
        assert_eq!(store.insert_genesis(g), Err(ChainError::InvalidGenesis));
    }

    #[test]
    fn genesis_non_zero_parent_rejected() {
        let store = BlockStore::new(poa());
        let mut g = genesis();
        g.header.parent_hash = H256::from_bytes([9u8; 32]);
        poa()
            .seal_header(&mut g.header, &miner(), &miner_sk())
            .unwrap();
        assert_eq!(store.insert_genesis(g), Err(ChainError::InvalidGenesis));
    }

    #[test]
    fn linear_chain_three_blocks() {
        let store = BlockStore::new(poa());
        let g = genesis();
        let h0 = store.insert_genesis(g.clone()).unwrap();
        let b1 = blk(1, h0, 2);
        let h1 = store.insert_block(b1).unwrap();
        let b2 = blk(2, h1, 3);
        let h2 = store.insert_block(b2).unwrap();
        assert_eq!(store.head(), Some(h2));
        assert_eq!(store.get_block_by_number(0).unwrap().hash(), h0);
        assert_eq!(store.get_block_by_number(1).unwrap().hash(), h1);
        assert_eq!(store.get_block_by_number(2).unwrap().hash(), h2);
    }

    #[test]
    fn duplicate_hash_rejected() {
        let store = BlockStore::new(poa());
        let g = genesis();
        store.insert_genesis(g.clone()).unwrap();
        assert_eq!(store.insert_genesis(g), Err(ChainError::DuplicateBlock));
    }

    #[test]
    fn unknown_parent_rejected() {
        let store = BlockStore::new(poa());
        store.insert_genesis(genesis()).unwrap();
        let orphan = blk(1, H256::from_bytes([3u8; 32]), 2);
        assert_eq!(store.insert_block(orphan), Err(ChainError::UnknownParent));
    }

    #[test]
    fn wrong_number_rejected() {
        let store = BlockStore::new(poa());
        let h0 = store.insert_genesis(genesis()).unwrap();
        let bad = blk(2, h0, 2);
        assert_eq!(
            store.insert_block(bad),
            Err(ChainError::InvalidBlockNumber {
                expected: 1,
                got: 2
            })
        );
    }

    #[test]
    fn insert_block_zero_routes_to_genesis() {
        let store = BlockStore::new(poa());
        let hash = store.insert_block(genesis()).unwrap();
        assert_eq!(store.head(), Some(hash));
    }

    #[test]
    fn fork_longer_branch_becomes_head() {
        let store = BlockStore::new(poa());
        let h0 = store.insert_genesis(genesis()).unwrap();
        let a1 = blk(1, h0, 10);
        let ha1 = store.insert_block(a1).unwrap();
        // Side branch from genesis with a different timestamp so hashes differ.
        let b1 = blk(1, h0, 11);
        let hb1 = store.insert_block(b1).unwrap();
        assert_ne!(ha1, hb1);
        let b2 = blk(2, hb1, 12);
        let hb2 = store.insert_block(b2).unwrap();
        assert_eq!(store.head(), Some(hb2));
        assert_eq!(store.get_block_by_number(1).unwrap().hash(), hb1);
        assert_eq!(store.get_block_by_number(2).unwrap().hash(), hb2);
    }

    #[test]
    fn equal_height_prefers_smaller_hash() {
        let store = BlockStore::new(poa());
        let h0 = store.insert_genesis(genesis()).unwrap();
        let a = blk(1, h0, 10);
        let b = blk(1, h0, 11);
        let ha = a.hash();
        let hb = b.hash();
        let (first, second, smaller) = if ha < hb { (a, b, ha) } else { (b, a, hb) };
        store.insert_block(first).unwrap();
        store.insert_block(second).unwrap();
        assert_eq!(store.head(), Some(smaller));
        assert_eq!(store.get_block_by_number(1).unwrap().hash(), smaller);
    }

    #[test]
    fn missing_get_is_none() {
        let store = BlockStore::new(poa());
        assert!(store.get_block(&H256::ZERO).is_none());
        assert!(store.get_block_by_number(0).is_none());
        assert!(store.head().is_none());
        assert!(store.state_at_block(0).is_none());
    }

    #[test]
    fn record_state_snapshot() {
        let store = BlockStore::new(poa());
        store.insert_genesis(genesis()).unwrap();
        let db = StateDB::new();
        store.record_state(0, db.clone());
        assert!(store.state_at_block(0).is_some());
    }

    #[test]
    fn extra_data_without_seal_rejected() {
        let store = BlockStore::new(poa());
        let mut g = genesis();
        g.header.extra_data = Bytes::new();
        assert!(store.insert_genesis(g).is_err());
    }

    #[test]
    fn encode_seals_used_by_store_path() {
        let extra = encode_seals(1).unwrap();
        assert_eq!(extra.as_slice().len(), 64);
    }
}
