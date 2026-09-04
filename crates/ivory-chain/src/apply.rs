//! Import a sealed block: verify `state_root`, then follow the canonical head.

use std::collections::HashSet;

use ivory_core::{Block, Transaction};
use ivory_executor::{ExecutionContext, Executor};
use ivory_primitives::H256;
use ivory_state::StateDB;
use ivory_txpool::{TransactionPool, TxOrigin};

use crate::error::ChainError;
use crate::store::{BlockStore, InsertOutcome};

/// Execute `block` on a fork of the parent snapshot, check `state_root`, insert,
/// and if the head moved, reset `live` and fix the mempool.
///
/// # Errors
///
/// Unknown parent / parent snapshot, execution failure, or `state_root` mismatch.
pub fn import_and_apply(
    store: &BlockStore,
    live: &StateDB,
    pool: &TransactionPool,
    block: Block,
) -> Result<InsertOutcome, ChainError> {
    let parent = block.header.parent_hash;
    let parent_state = store
        .state_at(&parent)
        .ok_or(ChainError::UnknownParentState)?;
    let trial = parent_state;
    let exec = Executor::new(trial.clone());
    let mut ctx = ExecutionContext::new(block.header.number, block.header.timestamp);
    for tx in &block.transactions {
        exec.execute_transaction(tx, &mut ctx)?;
    }
    if trial.root_hash() != block.header.state_root {
        return Err(ChainError::InvalidStateRoot);
    }
    let outcome = store.insert_block(block.clone())?;
    store.record_state(outcome.hash, trial.fork());
    if outcome.head_changed {
        apply_head_change(store, live, pool, &outcome);
    }
    Ok(outcome)
}

fn apply_head_change(
    store: &BlockStore,
    live: &StateDB,
    pool: &TransactionPool,
    outcome: &InsertOutcome,
) {
    if let Some(post) = store.state_at(&outcome.new_head) {
        live.reset_from(&post);
    }
    let old_txs = outcome
        .old_head
        .map(|h| txs_on_path(store, h))
        .unwrap_or_default();
    let new_txs = txs_on_path(store, outcome.new_head);
    let new_hashes: HashSet<H256> = new_txs.iter().map(Transaction::hash).collect();
    for tx in old_txs {
        if !new_hashes.contains(&tx.hash()) {
            let _ = pool.add_transaction(tx, TxOrigin::Local);
        }
    }
    for tx in new_txs {
        pool.remove(&tx.hash());
    }
}

fn txs_on_path(store: &BlockStore, tip: H256) -> Vec<Transaction> {
    let mut out = Vec::new();
    for hash in store.chain_from(tip) {
        if let Some(block) = store.get_block(&hash) {
            out.extend(block.transactions);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use ivory_consensus::{ConsensusEngine, PoAConsensus};
    use ivory_core::{Account, Block, BlockHeader, empty_list_roots};
    use ivory_crypto::{keypair_from_byte, signed_transfer};
    use ivory_primitives::{Address, Bytes, H256, SecretKey, U256};
    use ivory_state::StateDB;

    use super::*;
    use crate::producer::{BlockProducer, ProduceParams};
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

    fn funded(balance: u64) -> Account {
        let mut a = Account::new();
        a.balance = U256::from(balance);
        a
    }

    fn genesis(state: &StateDB) -> Block {
        let (tx_root, rx_root) = empty_list_roots();
        let mut header = BlockHeader {
            number: 0,
            parent_hash: H256::ZERO,
            timestamp: 1,
            miner: miner(),
            gas_limit: 30_000_000,
            gas_used: 0,
            state_root: state.root_hash(),
            transactions_root: tx_root,
            receipts_root: rx_root,
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

    fn transfer(from_seed: u8, to_seed: u8, nonce: u64) -> Transaction {
        signed_transfer(
            &keypair_from_byte(from_seed).0,
            addr(to_seed),
            nonce,
            U256::from(10u64),
            21_000,
        )
    }

    fn produce(parent: &Block, parent_state: StateDB, tx: Option<Transaction>, ts: u64) -> Block {
        let pool = TransactionPool::new();
        if let Some(tx) = tx {
            pool.add_transaction(tx, TxOrigin::Local).unwrap();
        }
        let exec = Executor::new(parent_state);
        BlockProducer::new()
            .produce_block(ProduceParams {
                parent,
                pool: &pool,
                executor: &exec,
                consensus: &poa(),
                miner: miner(),
                miner_key: &miner_sk(),
                timestamp: ts,
                max_txs: 8,
            })
            .unwrap()
    }

    fn setup() -> (BlockStore, StateDB, Block) {
        let state = StateDB::new();
        state.set_account(addr(1), funded(1_000_000));
        let g = genesis(&state);
        let store = BlockStore::new(poa());
        store.insert_genesis(g.clone()).unwrap();
        store.record_state(g.hash(), state.fork());
        let live = state.fork();
        (store, live, g)
    }

    #[test]
    fn reorg_restores_balances() {
        let (store, live, g) = setup();
        let pool = TransactionPool::new();
        let block_a = produce(
            &g,
            store.state_at(&g.hash()).unwrap(),
            Some(transfer(1, 2, 0)),
            10,
        );
        let block_b = produce(
            &g,
            store.state_at(&g.hash()).unwrap(),
            Some(transfer(1, 3, 0)),
            11,
        );
        import_and_apply(&store, &live, &pool, block_a.clone()).unwrap();
        import_and_apply(&store, &live, &pool, block_b.clone()).unwrap();

        let block_b2 = produce(&block_b, store.state_at(&block_b.hash()).unwrap(), None, 12);
        import_and_apply(&store, &live, &pool, block_b2.clone()).unwrap();

        assert_eq!(store.head(), Some(block_b2.hash()));
        assert_eq!(
            live.get_account(&addr(3)).map(|a| a.balance),
            Some(U256::from(10u64))
        );
        assert!(
            live.get_account(&addr(2))
                .is_none_or(|a| a.balance.is_zero())
        );
        assert_eq!(live.get_account(&addr(1)).unwrap().nonce, 1);
    }

    #[test]
    fn bad_state_root_rejected() {
        let (store, live, g) = setup();
        let pool = TransactionPool::new();
        let mut block = produce(
            &g,
            store.state_at(&g.hash()).unwrap(),
            Some(transfer(1, 2, 0)),
            10,
        );
        block.header.state_root = H256::from_bytes([0x11; 32]);
        poa()
            .seal_header(&mut block.header, &miner(), &miner_sk())
            .unwrap();
        assert_eq!(
            import_and_apply(&store, &live, &pool, block),
            Err(ChainError::InvalidStateRoot)
        );
        assert_eq!(
            live.get_account(&addr(1)).unwrap().balance,
            U256::from(1_000_000u64)
        );
    }
}
