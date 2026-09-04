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

/// Result of inserting a block into [`BlockStore`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InsertOutcome {
    /// Hash of the inserted block.
    pub hash: H256,
    /// Canonical head before this insert.
    pub old_head: Option<H256>,
    /// Canonical head after this insert.
    pub new_head: H256,
    /// Common ancestor of `old_head` and `new_head` (the new block if first).
    pub ancestor: H256,
    /// Whether the canonical head moved.
    pub head_changed: bool,
}

/// Indexed blocks plus optional per-block state snapshots.
pub struct BlockStore {
    by_hash: RwLock<HashMap<H256, Block>>,
    /// Canonical hash at each height (longest-chain view).
    by_number: RwLock<HashMap<u64, H256>>,
    head: RwLock<Option<H256>>,
    snapshots: RwLock<HashMap<H256, StateDB>>,
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

    /// Isolated snapshot recorded after `hash` was applied.
    #[must_use]
    pub fn state_at(&self, hash: &H256) -> Option<StateDB> {
        self.snapshots.read().get(hash).map(StateDB::fork)
    }

    /// Store a deep copy of `state` keyed by block hash.
    pub fn record_state(&self, hash: H256, state: StateDB) {
        self.snapshots.write().insert(hash, state.fork());
    }

    /// Drop transaction/receipt bodies below `keep_from` unless `archive`.
    pub fn drop_bodies_below(&self, keep_from: u64, archive: bool) {
        if archive {
            return;
        }
        let mut by_hash = self.by_hash.write();
        for block in by_hash.values_mut() {
            if block.header.number < keep_from {
                block.transactions.clear();
                block.receipts.clear();
            }
        }
    }

    /// Drop snapshots that are not on the canonical path.
    pub fn prune_snapshots(&self) {
        let Some(head) = self.head() else {
            return;
        };
        let keep: std::collections::HashSet<H256> = self.chain_from(head).into_iter().collect();
        self.snapshots.write().retain(|h, _| keep.contains(h));
    }

    /// Canonical snapshots only, then drop bodies/snapshots older than `keep` heights.
    pub fn prune_snapshots_keep(&self, keep: u64) {
        self.prune_snapshots();
        let Some(head) = self.head_block() else {
            return;
        };
        let cutoff = head.header.number.saturating_sub(keep.saturating_sub(1));
        self.drop_bodies_below(cutoff, false);
        let keep_hashes: std::collections::HashSet<H256> = self
            .chain_from(head.hash())
            .into_iter()
            .filter(|h| self.get_block(h).is_some_and(|b| b.header.number >= cutoff))
            .collect();
        self.snapshots
            .write()
            .retain(|h, _| keep_hashes.contains(h));
    }

    /// Genesis-to-tip hashes for `tip` (parent walk).
    #[must_use]
    pub fn chain_from(&self, tip: H256) -> Vec<H256> {
        let mut path = Vec::new();
        let mut cur = Some(tip);
        let by_hash = self.by_hash.read();
        while let Some(h) = cur {
            let Some(block) = by_hash.get(&h) else {
                break;
            };
            path.push(h);
            if block.header.number == 0 {
                break;
            }
            cur = Some(block.header.parent_hash);
        }
        path.reverse();
        path
    }

    /// Deepest block that is an ancestor of both tips.
    #[must_use]
    pub fn common_ancestor(&self, a: H256, b: H256) -> Option<H256> {
        let sa: std::collections::HashSet<H256> = self.chain_from(a).into_iter().collect();
        self.chain_from(b)
            .into_iter()
            .rev()
            .find(|h| sa.contains(h))
    }

    /// Insert genesis (`number == 0`, `parent_hash == ZERO`).
    ///
    /// # Errors
    ///
    /// [`ChainError::InvalidGenesis`], consensus, or duplicate hash.
    pub fn insert_genesis(&self, block: Block) -> Result<InsertOutcome, ChainError> {
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
    pub fn insert_block(&self, block: Block) -> Result<InsertOutcome, ChainError> {
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

    fn insert_validated(&self, block: Block) -> Result<InsertOutcome, ChainError> {
        let hash = block.hash();
        if self.by_hash.read().contains_key(&hash) {
            return Err(ChainError::DuplicateBlock);
        }
        let number = block.header.number;
        let old_head = *self.head.read();
        self.by_hash.write().insert(hash, block);
        self.maybe_update_canonical(hash, number);
        let new_head = self.head().expect("head after insert");
        let head_changed = old_head != Some(new_head);
        let ancestor = match old_head {
            None => hash,
            Some(old) if !head_changed => old,
            Some(old) => self.common_ancestor(old, new_head).unwrap_or(hash),
        };
        if head_changed {
            self.rebuild_tx_index();
        }
        Ok(InsertOutcome {
            hash,
            old_head,
            new_head,
            ancestor,
            head_changed,
        })
    }

    fn rebuild_tx_index(&self) {
        let mut idx = HashMap::new();
        let Some(head) = self.head_block() else {
            *self.tx_index.write() = idx;
            return;
        };
        for n in 0..=head.header.number {
            let Some(block) = self.get_block_by_number(n) else {
                continue;
            };
            let hash = block.hash();
            for (index, tx) in block.transactions.iter().enumerate() {
                idx.insert(
                    tx.hash(),
                    TxLocation {
                        block_hash: hash,
                        block_number: n,
                        index,
                    },
                );
            }
        }
        *self.tx_index.write() = idx;
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
    use ivory_core::{Block, BlockHeader, empty_list_roots};
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
        let (tx_root, rx_root) = empty_list_roots();
        let mut h = BlockHeader {
            number,
            parent_hash: parent,
            timestamp: ts,
            miner: miner(),
            gas_limit: 30_000_000,
            gas_used: 0,
            state_root: H256::ZERO,
            transactions_root: tx_root,
            receipts_root: rx_root,
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
        let hash = store.insert_genesis(g.clone()).unwrap().hash;
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
        let h0 = store.insert_genesis(g.clone()).unwrap().hash;
        let b1 = blk(1, h0, 2);
        let h1 = store.insert_block(b1).unwrap().hash;
        let b2 = blk(2, h1, 3);
        let h2 = store.insert_block(b2).unwrap().hash;
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
        let h0 = store.insert_genesis(genesis()).unwrap().hash;
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
        let hash = store.insert_block(genesis()).unwrap().hash;
        assert_eq!(store.head(), Some(hash));
    }

    #[test]
    fn fork_longer_branch_becomes_head() {
        let store = BlockStore::new(poa());
        let h0 = store.insert_genesis(genesis()).unwrap().hash;
        let a1 = blk(1, h0, 10);
        let ha1 = store.insert_block(a1).unwrap().hash;
        // Side branch from genesis with a different timestamp so hashes differ.
        let b1 = blk(1, h0, 11);
        let hb1 = store.insert_block(b1).unwrap().hash;
        assert_ne!(ha1, hb1);
        let b2 = blk(2, hb1, 12);
        let hb2 = store.insert_block(b2).unwrap().hash;
        assert_eq!(store.head(), Some(hb2));
        assert_eq!(store.get_block_by_number(1).unwrap().hash(), hb1);
        assert_eq!(store.get_block_by_number(2).unwrap().hash(), hb2);
    }

    #[test]
    fn equal_height_prefers_smaller_hash() {
        let store = BlockStore::new(poa());
        let h0 = store.insert_genesis(genesis()).unwrap().hash;
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
        assert!(store.state_at(&H256::ZERO).is_none());
    }

    #[test]
    fn record_state_snapshot() {
        let store = BlockStore::new(poa());
        let hash = store.insert_genesis(genesis()).unwrap().hash;
        let db = StateDB::new();
        store.record_state(hash, db);
        assert!(store.state_at(&hash).is_some());
    }

    #[test]
    fn insert_reports_reorg_ancestor() {
        let store = BlockStore::new(poa());
        let h0 = store.insert_genesis(genesis()).unwrap().hash;
        let a1 = store.insert_block(blk(1, h0, 10)).unwrap();
        let b1 = store.insert_block(blk(1, h0, 11)).unwrap();
        let (head1, other1) = if store.head() == Some(a1.hash) {
            (a1, b1)
        } else {
            (b1, a1)
        };
        let taller = store.insert_block(blk(2, other1.hash, 12)).unwrap();
        assert!(taller.head_changed);
        assert_eq!(taller.old_head, Some(head1.hash));
        assert_eq!(taller.new_head, taller.hash);
        assert_eq!(taller.ancestor, h0);
    }

    #[test]
    fn prune_snapshots_drops_side_fork() {
        let store = BlockStore::new(poa());
        let h0 = store.insert_genesis(genesis()).unwrap().hash;
        store.record_state(h0, StateDB::new());
        let a1 = store.insert_block(blk(1, h0, 10)).unwrap().hash;
        store.record_state(a1, StateDB::new());
        let b1 = store.insert_block(blk(1, h0, 11)).unwrap().hash;
        store.record_state(b1, StateDB::new());
        let taller_parent = if store.head() == Some(a1) { b1 } else { a1 };
        let h2 = store.insert_block(blk(2, taller_parent, 12)).unwrap().hash;
        store.record_state(h2, StateDB::new());
        store.prune_snapshots();
        assert!(store.state_at(&h2).is_some());
        let loser = if taller_parent == a1 { b1 } else { a1 };
        assert!(store.state_at(&loser).is_none());
    }

    #[test]
    fn prune_snapshots_keep_drops_old_canonical_snapshot() {
        let store = BlockStore::new(poa());
        let h0 = store.insert_genesis(genesis()).unwrap().hash;
        store.record_state(h0, StateDB::new());
        let h1 = store.insert_block(blk(1, h0, 2)).unwrap().hash;
        store.record_state(h1, StateDB::new());
        let h2 = store.insert_block(blk(2, h1, 3)).unwrap().hash;
        store.record_state(h2, StateDB::new());
        store.prune_snapshots_keep(1);
        assert!(store.state_at(&h0).is_none());
        assert!(store.state_at(&h2).is_some());
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
