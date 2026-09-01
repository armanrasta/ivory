//! Assemble a block from the mempool and executor.

use ivory_consensus::{ConsensusEngine, PoAConsensus};
use ivory_core::{Block, BlockHeader, Receipt, Transaction};
use ivory_executor::{ExecutionContext, Executor, GasConfig};
use ivory_primitives::{Address, H256, U256};
use ivory_txpool::TransactionPool;

use crate::error::ChainError;

/// Inputs for [`BlockProducer::produce_block`].
pub struct ProduceParams<'a> {
    /// Parent of the block being built.
    pub parent: &'a Block,
    /// Pending transactions.
    pub pool: &'a TransactionPool,
    /// Shared executor / state.
    pub executor: &'a Executor,
    /// Seals the header.
    pub consensus: &'a PoAConsensus,
    /// Block miner (must be a validator).
    pub miner: Address,
    /// Header timestamp.
    pub timestamp: u64,
    /// Cap on `pool.get_pending`.
    pub max_txs: usize,
}

/// Builds sealed blocks from pending transactions.
pub struct BlockProducer {
    gas_limit: u64,
}

impl Default for BlockProducer {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockProducer {
    /// Use [`GasConfig::default`] block gas cap as the header `gas_limit`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            gas_limit: GasConfig::default().max_gas_per_block,
        }
    }

    /// Custom header gas limit.
    #[must_use]
    pub fn with_gas_limit(gas_limit: u64) -> Self {
        Self { gas_limit }
    }

    /// Pull up to `max_txs` from `pool`, execute in `(from, nonce)` order, seal.
    ///
    /// Transactions that fail execution are skipped (v1 mempool honesty).
    ///
    /// # Errors
    ///
    /// [`ChainError::Consensus`] if `miner` cannot seal.
    pub fn produce_block(&self, params: ProduceParams<'_>) -> Result<Block, ChainError> {
        let ProduceParams {
            parent,
            pool,
            executor,
            consensus,
            miner,
            timestamp,
            max_txs,
        } = params;
        let mut pending = pool.get_pending(max_txs);
        pending.sort_by(|a, b| a.from.cmp(&b.from).then(a.nonce.cmp(&b.nonce)));

        let mut ctx = ExecutionContext::new(parent.header.number.saturating_add(1), timestamp);
        ctx.beneficiary = miner;

        let mut included: Vec<Transaction> = Vec::new();
        let mut receipts: Vec<Receipt> = Vec::new();
        for tx in pending {
            match executor.execute_transaction(&tx, &mut ctx) {
                Ok(out) => {
                    included.push(tx);
                    receipts.push(out.receipt);
                }
                Err(_) => continue,
            }
        }

        let mut header = BlockHeader {
            number: parent.header.number.saturating_add(1),
            parent_hash: parent.hash(),
            timestamp,
            miner,
            gas_limit: self.gas_limit,
            gas_used: ctx.gas_used,
            state_root: executor.state().root_hash(),
            transactions_root: H256::ZERO,
            receipts_root: H256::ZERO,
            difficulty: U256::ZERO,
            extra_data: ivory_primitives::Bytes::new(),
        };
        consensus.seal_header(&mut header, &miner)?;

        Ok(Block {
            header,
            transactions: included,
            receipts,
        })
    }
}

#[cfg(test)]
mod tests {
    use ivory_consensus::PoAConsensus;
    use ivory_core::{Account, Block, BlockHeader, Transaction};
    use ivory_primitives::{Address, Bytes, H256, Signature, U256};
    use ivory_state::StateDB;
    use ivory_txpool::{TransactionPool, TxOrigin};

    use super::*;
    use crate::store::BlockStore;

    fn miner() -> Address {
        Address::from_bytes([9u8; 20])
    }

    fn addr(b: u8) -> Address {
        Address::from_bytes([b; 20])
    }

    fn poa() -> PoAConsensus {
        PoAConsensus::with_validator(miner()).unwrap()
    }

    fn genesis_block() -> Block {
        let mut header = BlockHeader {
            number: 0,
            parent_hash: H256::ZERO,
            timestamp: 1,
            miner: miner(),
            gas_limit: 30_000_000,
            gas_used: 0,
            state_root: H256::ZERO,
            transactions_root: H256::ZERO,
            receipts_root: H256::ZERO,
            difficulty: U256::ZERO,
            extra_data: Bytes::new(),
        };
        poa().seal_header(&mut header, &miner()).unwrap();
        Block {
            header,
            transactions: Vec::new(),
            receipts: Vec::new(),
        }
    }

    fn transfer(from: Address, to: Address, nonce: u64) -> Transaction {
        Transaction {
            from,
            to: Some(to),
            value: U256::from(10u64),
            data: Bytes::new(),
            gas_price: U256::ONE,
            gas: 21_000,
            nonce,
            signature: Signature::zero(),
        }
    }

    fn funded(balance: u64) -> Account {
        let mut a = Account::new();
        a.balance = U256::from(balance);
        a
    }

    fn params<'a>(
        parent: &'a Block,
        pool: &'a TransactionPool,
        exec: &'a Executor,
        consensus: &'a PoAConsensus,
        miner: Address,
        timestamp: u64,
    ) -> ProduceParams<'a> {
        ProduceParams {
            parent,
            pool,
            executor: exec,
            consensus,
            miner,
            timestamp,
            max_txs: 8,
        }
    }

    #[test]
    fn produce_empty_block() {
        let parent = genesis_block();
        let pool = TransactionPool::new();
        let exec = Executor::new(StateDB::new());
        let poa = poa();
        let block = BlockProducer::new()
            .produce_block(params(&parent, &pool, &exec, &poa, miner(), 2))
            .unwrap();
        assert_eq!(block.header.number, 1);
        assert_eq!(block.header.parent_hash, parent.hash());
        assert!(block.transactions.is_empty());
        assert_eq!(block.header.gas_used, 0);
        poa.validate_header(&block.header, Some(&parent.header))
            .unwrap();
    }

    #[test]
    fn produce_rejects_non_validator_miner() {
        let parent = genesis_block();
        let pool = TransactionPool::new();
        let exec = Executor::new(StateDB::new());
        let poa = poa();
        let err = BlockProducer::new()
            .produce_block(params(&parent, &pool, &exec, &poa, addr(3), 2))
            .unwrap_err();
        assert!(matches!(err, ChainError::Consensus(_)));
    }

    #[test]
    fn produce_includes_two_transfers() {
        let state = StateDB::new();
        let from = addr(1);
        let to = addr(2);
        state.set_account(from, funded(1_000_000));
        let pool = TransactionPool::new();
        pool.add_transaction(transfer(from, to, 0), TxOrigin::Local)
            .unwrap();
        pool.add_transaction(transfer(from, to, 1), TxOrigin::Local)
            .unwrap();
        let exec = Executor::new(state);
        let parent = genesis_block();
        let block = BlockProducer::new()
            .produce_block(params(&parent, &pool, &exec, &poa(), miner(), 2))
            .unwrap();
        assert_eq!(block.transactions.len(), 2);
        assert_eq!(block.receipts.len(), 2);
        assert_eq!(block.header.gas_used, 42_000);
        assert_eq!(exec.state().get_account(&from).unwrap().nonce, 2);
        assert_eq!(
            exec.state().get_account(&to).unwrap().balance,
            U256::from(20u64)
        );
    }

    #[test]
    fn produce_skips_invalid_tx() {
        let state = StateDB::new();
        let from = addr(1);
        state.set_account(from, funded(1_000_000));
        let pool = TransactionPool::new();
        // Wrong nonce relative to account (account nonce 0, tx nonce 5) — pool
        // may still hold it if admitted as first tx; use nonce 0 then a gap-free
        // second that fails balance by using a huge value after first spends.
        pool.add_transaction(transfer(from, addr(2), 0), TxOrigin::Local)
            .unwrap();
        let mut expensive = transfer(from, addr(2), 1);
        expensive.value = U256::from(u64::MAX);
        pool.add_transaction(expensive, TxOrigin::Local).unwrap();
        let exec = Executor::new(state);
        let parent = genesis_block();
        let block = BlockProducer::new()
            .produce_block(params(&parent, &pool, &exec, &poa(), miner(), 2))
            .unwrap();
        assert_eq!(block.transactions.len(), 1);
        assert_eq!(exec.state().get_account(&from).unwrap().nonce, 1);
    }

    #[test]
    fn default_producer_matches_new() {
        assert_eq!(
            BlockProducer::default().gas_limit,
            BlockProducer::new().gas_limit
        );
    }

    #[test]
    fn with_gas_limit() {
        assert_eq!(BlockProducer::with_gas_limit(1_000).gas_limit, 1_000);
    }

    #[test]
    fn produce_then_insert() {
        let store = BlockStore::new(poa());
        let g = genesis_block();
        store.insert_genesis(g.clone()).unwrap();
        let state = StateDB::new();
        state.set_account(addr(1), funded(1_000_000));
        let pool = TransactionPool::new();
        pool.add_transaction(transfer(addr(1), addr(2), 0), TxOrigin::Local)
            .unwrap();
        let exec = Executor::new(state);
        let block = BlockProducer::new()
            .produce_block(params(&g, &pool, &exec, &poa(), miner(), 2))
            .unwrap();
        let hash = store.insert_block(block).unwrap();
        assert_eq!(store.head(), Some(hash));
        store.record_state(1, exec.state().clone());
        assert!(store.state_at_block(1).is_some());
    }
}
