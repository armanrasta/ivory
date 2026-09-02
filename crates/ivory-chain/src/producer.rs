//! Assemble a block from the mempool and executor.

use ivory_consensus::{ConsensusEngine, PoAConsensus};
use ivory_core::{Block, BlockHeader, Receipt, Transaction};
use ivory_executor::{ExecutionContext, Executor, GasConfig};
use ivory_primitives::{Address, H256, SecretKey, U256};
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
    /// Miner Ed25519 secret used to seal the header.
    pub miner_key: &'a SecretKey,
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
    /// Signatures are checked at pool admission, not here.
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
            miner_key,
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
        consensus.seal_header(&mut header, &miner, miner_key)?;

        Ok(Block {
            header,
            transactions: included,
            receipts,
        })
    }
}

#[cfg(test)]
mod tests {
    use ivory_consensus::{ConsensusEngine, PoAConsensus};
    use ivory_core::{Account, Block, BlockHeader};
    use ivory_crypto::{keypair_from_byte, signed_transfer, signed_tx};
    use ivory_primitives::{Address, Bytes, H256, SecretKey, U256};
    use ivory_state::StateDB;
    use ivory_txpool::{TransactionPool, TxOrigin};

    use super::*;
    use crate::store::BlockStore;

    fn miner_sk() -> SecretKey {
        keypair_from_byte(9).0
    }

    fn miner() -> Address {
        keypair_from_byte(9).2
    }

    fn addr(b: u8) -> Address {
        keypair_from_byte(b).2
    }

    fn poa() -> PoAConsensus {
        PoAConsensus::from_secret(&miner_sk()).unwrap()
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
        poa()
            .seal_header(&mut header, &miner(), &miner_sk())
            .unwrap();
        Block {
            header,
            transactions: Vec::new(),
            receipts: Vec::new(),
        }
    }

    fn transfer(from_seed: u8, to_seed: u8, nonce: u64) -> ivory_core::Transaction {
        signed_transfer(
            &keypair_from_byte(from_seed).0,
            addr(to_seed),
            nonce,
            U256::from(10u64),
            21_000,
        )
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
        miner_key: &'a SecretKey,
        timestamp: u64,
    ) -> ProduceParams<'a> {
        ProduceParams {
            parent,
            pool,
            executor: exec,
            consensus,
            miner,
            miner_key,
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
        let miner_key = miner_sk();
        let block = BlockProducer::new()
            .produce_block(params(&parent, &pool, &exec, &poa, miner(), &miner_key, 2))
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
        let other = keypair_from_byte(3).0;
        let err = BlockProducer::new()
            .produce_block(params(&parent, &pool, &exec, &poa, addr(3), &other, 2))
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
        pool.add_transaction(transfer(1, 2, 0), TxOrigin::Local)
            .unwrap();
        pool.add_transaction(transfer(1, 2, 1), TxOrigin::Local)
            .unwrap();
        let exec = Executor::new(state);
        let parent = genesis_block();
        let miner_key = miner_sk();
        let block = BlockProducer::new()
            .produce_block(params(
                &parent,
                &pool,
                &exec,
                &poa(),
                miner(),
                &miner_key,
                2,
            ))
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
        pool.add_transaction(transfer(1, 2, 0), TxOrigin::Local)
            .unwrap();
        let expensive = signed_tx(
            &keypair_from_byte(1).0,
            Some(addr(2)),
            1,
            U256::from(u64::MAX),
            21_000,
            U256::ONE,
            Bytes::new(),
        );
        pool.add_transaction(expensive, TxOrigin::Local).unwrap();
        let exec = Executor::new(state);
        let parent = genesis_block();
        let miner_key = miner_sk();
        let block = BlockProducer::new()
            .produce_block(params(
                &parent,
                &pool,
                &exec,
                &poa(),
                miner(),
                &miner_key,
                2,
            ))
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
        pool.add_transaction(transfer(1, 2, 0), TxOrigin::Local)
            .unwrap();
        let exec = Executor::new(state);
        let miner_key = miner_sk();
        let block = BlockProducer::new()
            .produce_block(params(&g, &pool, &exec, &poa(), miner(), &miner_key, 2))
            .unwrap();
        let hash = store.insert_block(block).unwrap();
        assert_eq!(store.head(), Some(hash));
        store.record_state(1, exec.state().clone());
        assert!(store.state_at_block(1).is_some());
    }
}
