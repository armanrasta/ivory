//! Pool → produce → insert against a shared `StateDB`.

use ivory_chain::{BlockProducer, BlockStore, ProduceParams};
use ivory_consensus::{ConsensusEngine, PoAConsensus};
use ivory_core::{Account, Block, BlockHeader};
use ivory_crypto::{keypair_from_byte, signed_transfer};
use ivory_executor::Executor;
use ivory_primitives::{Address, Bytes, H256, SecretKey, U256};
use ivory_state::StateDB;
use ivory_txpool::{TransactionPool, TxOrigin};

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
    pool.add_transaction(transfer(1, 2, 0), TxOrigin::Local)
        .unwrap();
    pool.add_transaction(transfer(1, 2, 1), TxOrigin::Local)
        .unwrap();

    let exec = Executor::new(state);
    let miner_key = miner_sk();
    let block = BlockProducer::new()
        .produce_block(ProduceParams {
            parent: &g,
            pool: &pool,
            executor: &exec,
            consensus: &poa(),
            miner: miner(),
            miner_key: &miner_key,
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
