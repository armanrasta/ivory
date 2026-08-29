//! Pool admission then execution against a shared `StateDB`.

use ivory_core::Account;
use ivory_executor::{ExecutionContext, Executor};
use ivory_primitives::{Address, Bytes, Signature, U256};
use ivory_state::StateDB;
use ivory_txpool::{TransactionPool, TxOrigin};

fn addr(b: u8) -> Address {
    Address::from_bytes([b; 20])
}

fn transfer(from: Address, to: Address, nonce: u64) -> ivory_core::Transaction {
    ivory_core::Transaction {
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
fn pool_pending_then_execute() {
    let state = StateDB::new();
    let from = addr(1);
    let to = addr(2);
    let mut account = Account::new();
    account.balance = U256::from(1_000_000u64);
    state.set_account(from, account);

    let pool = TransactionPool::new();
    let tx0 = transfer(from, to, 0);
    let tx1 = transfer(from, to, 1);
    pool.add_transaction(tx0, TxOrigin::Local).unwrap();
    pool.add_transaction(tx1, TxOrigin::Local).unwrap();
    assert_eq!(pool.pending_count(), 2);

    let pending = pool.get_pending(8);
    assert_eq!(pending.len(), 2);

    let exec = Executor::new(state.clone());
    let mut ctx = ExecutionContext::new(1, 0);

    // Strict nonces: execute in nonce order regardless of get_pending order.
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
