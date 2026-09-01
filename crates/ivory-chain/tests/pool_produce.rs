//! Pool → produce → insert against a shared `StateDB`.

use ivory_chain::{BlockProducer, BlockStore, ProduceParams};
use ivory_consensus::{ConsensusEngine, PoAConsensus};
use ivory_core::{Account, Block, BlockHeader, Transaction};
use ivory_executor::Executor;
use ivory_primitives::{Address, Bytes, H256, Signature, U256};
use ivory_state::StateDB;
use ivory_txpool::{TransactionPool, TxOrigin};

fn miner() -> Address {
    Address::from_bytes([9u8; 20])
}

fn addr(b: u8) -> Address {
    Address::from_bytes([b; 20])
}

fn poa() -> PoAConsensus {
    PoAConsensus::with_validator(miner()).unwrap()
}

fn genesis() -> Block {
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

#[test]
fn pool_produce_insert() {
    let store = BlockStore::new(poa());
    let g = genesis();
    store.insert_genesis(g.clone()).unwrap();

    let state = StateDB::new();
    let from = addr(1);
    let to = addr(2);
    let mut account = Account::new();
    account.balance = U256::from(1_000_000u64);
    state.set_account(from, account);

    let pool = TransactionPool::new();
    pool.add_transaction(transfer(from, to, 0), TxOrigin::Local)
        .unwrap();
    pool.add_transaction(transfer(from, to, 1), TxOrigin::Local)
        .unwrap();

    let exec = Executor::new(state);
    let block = BlockProducer::new()
        .produce_block(ProduceParams {
            parent: &g,
            pool: &pool,
            executor: &exec,
            consensus: &poa(),
            miner: miner(),
            timestamp: 2,
            max_txs: 8,
        })
        .unwrap();
    assert_eq!(block.transactions.len(), 2);
    for tx in &block.transactions {
        pool.remove(&tx.hash());
    }
    assert_eq!(pool.pending_count(), 0);

    let hash = store.insert_block(block).unwrap();
    assert_eq!(store.head(), Some(hash));
    assert_eq!(store.get_block_by_number(1).unwrap().transactions.len(), 2);
    assert_eq!(exec.state().get_account(&from).unwrap().nonce, 2);
    assert_eq!(
        exec.state().get_account(&to).unwrap().balance,
        U256::from(20u64)
    );
}
