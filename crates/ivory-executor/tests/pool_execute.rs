//! Pool admission then execution against a shared `StateDB`.

use ivory_core::Account;
use ivory_crypto::{keypair_from_byte, signed_transfer};
use ivory_executor::{ExecutionContext, Executor};
use ivory_primitives::U256;
use ivory_state::StateDB;
use ivory_txpool::{TransactionPool, TxOrigin};

fn addr(b: u8) -> ivory_primitives::Address {
    keypair_from_byte(b).2
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
fn pool_pending_then_execute() {
    let state = StateDB::new();
    let from = addr(1);
    let to = addr(2);
    let mut account = Account::new();
    account.balance = U256::from(1_000_000u64);
    state.set_account(from, account);

    let pool = TransactionPool::new();
    let tx0 = transfer(1, 2, 0);
    let tx1 = transfer(1, 2, 1);
    pool.add_transaction(tx0, TxOrigin::Local).unwrap();
    pool.add_transaction(tx1, TxOrigin::Local).unwrap();
    assert_eq!(pool.pending_count(), 2);

    let pending = pool.get_pending(8);
    assert_eq!(pending.len(), 2);

    let exec = Executor::new(state.clone());
    let mut ctx = ExecutionContext::new(1, 0);

    let mut ordered = pending;
    ordered.sort_by_key(|t| t.nonce);
    for tx in &ordered {
        exec.execute_transaction(tx, &mut ctx).unwrap();
        pool.remove(&tx.hash());
    }

    assert_eq!(pool.pending_count(), 0);
    assert_eq!(exec.state().get_account(&from).unwrap().nonce, 2);
    assert_eq!(
        exec.state().get_account(&to).unwrap().balance,
        U256::from(20u64)
    );
    assert_eq!(ctx.gas_used, 42_000);
}
