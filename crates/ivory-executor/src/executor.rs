//! Transaction executor.

use ivory_core::{Receipt, Transaction};
use ivory_primitives::U256;
use ivory_state::StateDB;

use crate::call::{CallInput, CallResult, execute_call};
use crate::context::ExecutionContext;
use crate::error::ExecutionError;
use crate::gas::{GasConfig, GasMeter, compute_intrinsic_gas};

/// Receipt plus gas accounting for tests and block production.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionOutcome {
    /// Canonical receipt.
    pub receipt: Receipt,
    /// Gas charged for this transaction.
    pub gas_used: u64,
    /// Unused gas units refunded to the sender.
    pub gas_refunded: u64,
    /// Stub call result.
    pub call: CallResult,
}

/// Executes transfers and gas accounting against a [`StateDB`].
pub struct Executor {
    state: StateDB,
    gas_config: GasConfig,
}

impl Executor {
    /// Executor with default [`GasConfig`].
    #[must_use]
    pub fn new(state: StateDB) -> Self {
        Self::with_gas_config(state, GasConfig::default())
    }

    /// Executor with a custom gas schedule.
    #[must_use]
    pub fn with_gas_config(state: StateDB, gas_config: GasConfig) -> Self {
        Self { state, gas_config }
    }

    /// Shared state handle.
    #[must_use]
    pub fn state(&self) -> &StateDB {
        &self.state
    }

    /// Execute `tx` against `self.state`, updating `ctx.gas_used`.
    ///
    /// Signature verification is performed at pool admission; the executor
    /// trusts the caller (block producer / tests).
    ///
    /// Missing sender/recipient accounts are treated as empty (`Account::new()`).
    /// Contract creation (`to: None`) does not derive an address; endowment stays
    /// uncredited (documented stub until CREATE semantics in #16).
    ///
    /// # Errors
    ///
    /// [`ExecutionError`] on nonce, balance, gas, or overflow failures.
    pub fn execute_transaction(
        &self,
        tx: &Transaction,
        ctx: &mut ExecutionContext,
    ) -> Result<ExecutionOutcome, ExecutionError> {
        let mut sender = self.state.get_account(&tx.from).unwrap_or_default();

        if sender.nonce != tx.nonce {
            return Err(ExecutionError::NonceMismatch {
                expected: sender.nonce,
                got: tx.nonce,
            });
        }

        let intrinsic = compute_intrinsic_gas(tx, &self.gas_config);
        let meter = GasMeter::new(tx.gas, intrinsic)?;
        let gas_used = meter.gas_used();

        let gas_cost = U256::from(tx.gas)
            .checked_mul(tx.gas_price)
            .ok_or(ExecutionError::Overflow)?;
        let total = tx
            .value
            .checked_add(gas_cost)
            .ok_or(ExecutionError::Overflow)?;

        if sender.balance < total {
            return Err(ExecutionError::InsufficientBalance);
        }

        let projected = ctx.gas_used.saturating_add(tx.gas);
        if projected > self.gas_config.max_gas_per_block {
            return Err(ExecutionError::BlockGasLimitExceeded);
        }

        sender.balance = sender
            .balance
            .checked_sub(total)
            .ok_or(ExecutionError::Overflow)?;
        sender.nonce = sender
            .nonce
            .checked_add(1)
            .ok_or(ExecutionError::Overflow)?;
        self.state.set_account(tx.from, sender);

        // Call: credit recipient (empty account if missing). Create: endowment stays uncredited.
        if let Some(to) = tx.to {
            let mut recipient = self.state.get_account(&to).unwrap_or_default();
            recipient.balance = recipient
                .balance
                .checked_add(tx.value)
                .ok_or(ExecutionError::Overflow)?;
            self.state.set_account(to, recipient);
        }

        let input = CallInput::from_tx(tx);
        let call = execute_call(&input)?;

        let refund_units = meter.refund_gas();
        let refund = U256::from(refund_units)
            .checked_mul(tx.gas_price)
            .ok_or(ExecutionError::Overflow)?;
        if !refund.is_zero() {
            let mut sender = self.state.get_account(&tx.from).unwrap_or_default();
            sender.balance = sender
                .balance
                .checked_add(refund)
                .ok_or(ExecutionError::Overflow)?;
            self.state.set_account(tx.from, sender);
        }

        ctx.gas_used = ctx.gas_used.saturating_add(gas_used);

        let receipt = Receipt {
            tx_hash: tx.hash(),
            block_number: ctx.block_number,
            gas_used,
            status: call.success,
            logs: call.logs.clone(),
        };

        Ok(ExecutionOutcome {
            receipt,
            gas_used,
            gas_refunded: refund_units,
            call,
        })
    }
}

#[cfg(test)]
mod tests {
    use ivory_core::Account;
    use ivory_primitives::{Address, Bytes, PublicKey, Signature, U256};

    use super::*;
    use crate::{CallKind, GasConfig};

    fn addr(b: u8) -> Address {
        Address::from_bytes([b; 20])
    }

    fn funded(nonce: u64, balance: u64) -> Account {
        let mut a = Account::new();
        a.nonce = nonce;
        a.balance = U256::from(balance);
        a
    }

    fn transfer_tx(
        from: Address,
        to: Address,
        nonce: u64,
        value: u64,
        gas: u64,
        price: u64,
    ) -> Transaction {
        Transaction {
            from,
            to: Some(to),
            value: U256::from(value),
            data: Bytes::new(),
            gas_price: U256::from(price),
            gas,
            nonce,
            signature: Signature::zero(),
            public_key: PublicKey::zero(),
        }
    }

    fn create_tx(from: Address, nonce: u64, value: u64, gas: u64) -> Transaction {
        Transaction {
            from,
            to: None,
            value: U256::from(value),
            data: Bytes::from_slice(&[0x00]),
            gas_price: U256::from(1u64),
            gas,
            nonce,
            signature: Signature::zero(),
            public_key: PublicKey::zero(),
        }
    }

    #[test]
    fn happy_path_transfer() {
        let state = StateDB::new();
        let from = addr(1);
        let to = addr(2);
        state.set_account(from, funded(0, 1_000_000));
        let exec = Executor::new(state.clone());
        let tx = transfer_tx(from, to, 0, 100, 21_000, 2);
        let mut ctx = ExecutionContext::new(1, 1_700_000_000);
        let out = exec.execute_transaction(&tx, &mut ctx).unwrap();

        assert_eq!(out.receipt.tx_hash, tx.hash());
        assert!(out.receipt.status);
        assert_eq!(out.gas_used, 21_000);
        assert_eq!(out.gas_refunded, 0);
        assert!(out.call.success);
        assert_eq!(CallKind::from_tx(&tx), CallKind::Call);

        let sender = exec.state().get_account(&from).unwrap();
        assert_eq!(sender.nonce, 1);
        // paid value 100 + 21000*2 = 42100
        assert_eq!(sender.balance, U256::from(1_000_000u64 - 42_100));
        let recipient = exec.state().get_account(&to).unwrap();
        assert_eq!(recipient.balance, U256::from(100u64));
        assert_eq!(ctx.gas_used, 21_000);
    }

    #[test]
    fn nonce_mismatch() {
        let state = StateDB::new();
        let from = addr(1);
        state.set_account(from, funded(5, 1_000_000));
        let exec = Executor::new(state);
        let tx = transfer_tx(from, addr(2), 0, 1, 21_000, 1);
        let mut ctx = ExecutionContext::new(1, 0);
        assert_eq!(
            exec.execute_transaction(&tx, &mut ctx),
            Err(ExecutionError::NonceMismatch {
                expected: 5,
                got: 0
            })
        );
    }

    #[test]
    fn insufficient_balance() {
        let state = StateDB::new();
        let from = addr(1);
        state.set_account(from, funded(0, 10));
        let exec = Executor::new(state);
        let tx = transfer_tx(from, addr(2), 0, 1, 21_000, 1);
        let mut ctx = ExecutionContext::new(1, 0);
        assert_eq!(
            exec.execute_transaction(&tx, &mut ctx),
            Err(ExecutionError::InsufficientBalance)
        );
    }

    #[test]
    fn out_of_gas_intrinsic() {
        let state = StateDB::new();
        let from = addr(1);
        state.set_account(from, funded(0, 1_000_000));
        let exec = Executor::new(state);
        let mut tx = transfer_tx(from, addr(2), 0, 0, 21_000, 1);
        tx.data = Bytes::from_slice(&[1, 2, 3]);
        tx.gas = 21_000; // intrinsic = 21000 + 48
        let mut ctx = ExecutionContext::new(1, 0);
        assert_eq!(
            exec.execute_transaction(&tx, &mut ctx),
            Err(ExecutionError::OutOfGas)
        );
    }

    #[test]
    fn overflow_on_gas_cost() {
        let state = StateDB::new();
        let from = addr(1);
        let mut account = Account::new();
        account.balance = U256::MAX;
        state.set_account(from, account);
        let exec = Executor::new(state);
        let tx = Transaction {
            from,
            to: Some(addr(2)),
            value: U256::ZERO,
            data: Bytes::new(),
            gas_price: U256::MAX,
            gas: 21_000,
            nonce: 0,
            signature: Signature::zero(),
            public_key: PublicKey::zero(),
        };
        let mut ctx = ExecutionContext::new(1, 0);
        assert_eq!(
            exec.execute_transaction(&tx, &mut ctx),
            Err(ExecutionError::Overflow)
        );
    }

    #[test]
    fn missing_sender_empty_account_fails_balance() {
        let exec = Executor::new(StateDB::new());
        let tx = transfer_tx(addr(1), addr(2), 0, 1, 21_000, 1);
        let mut ctx = ExecutionContext::new(1, 0);
        assert_eq!(
            exec.execute_transaction(&tx, &mut ctx),
            Err(ExecutionError::InsufficientBalance)
        );
    }

    #[test]
    fn funded_then_execute() {
        let state = StateDB::new();
        state.set_account(addr(1), funded(0, 500_000));
        let exec = Executor::new(state);
        let tx = transfer_tx(addr(1), addr(2), 0, 50, 21_000, 1);
        let mut ctx = ExecutionContext::new(3, 0);
        let out = exec.execute_transaction(&tx, &mut ctx).unwrap();
        assert_eq!(out.receipt.block_number, 3);
    }

    #[test]
    fn create_path_does_not_credit_recipient() {
        let state = StateDB::new();
        let from = addr(1);
        state.set_account(from, funded(0, 1_000_000));
        let exec = Executor::new(state.clone());
        // intrinsic = 21000 + 16
        let tx = create_tx(from, 0, 100, 21_016);
        assert!(tx.is_create());
        assert_eq!(CallKind::from_tx(&tx), CallKind::Create);
        let mut ctx = ExecutionContext::new(1, 0);
        let out = exec.execute_transaction(&tx, &mut ctx).unwrap();
        assert!(out.receipt.status);
        assert_eq!(out.gas_used, 21_016);
        let sender = exec.state().get_account(&from).unwrap();
        assert_eq!(sender.nonce, 1);
        // value 100 stays uncredited; gas 21016 * 1, no refund
        assert_eq!(sender.balance, U256::from(1_000_000u64 - 21_016 - 100));
    }

    #[test]
    fn gas_refund_restores_unused() {
        let state = StateDB::new();
        let from = addr(1);
        state.set_account(from, funded(0, 1_000_000));
        let exec = Executor::new(state);
        let tx = transfer_tx(from, addr(2), 0, 0, 30_000, 3);
        let mut ctx = ExecutionContext::new(1, 0);
        let out = exec.execute_transaction(&tx, &mut ctx).unwrap();
        assert_eq!(out.gas_used, 21_000);
        assert_eq!(out.gas_refunded, 9_000);
        let sender = exec.state().get_account(&from).unwrap();
        // charged 30000*3 = 90000 then refund 9000*3 = 27000 → net 63000
        assert_eq!(sender.balance, U256::from(1_000_000u64 - 63_000));
    }

    #[test]
    fn cumulative_gas_increases() {
        let state = StateDB::new();
        let from = addr(1);
        state.set_account(from, funded(0, 10_000_000));
        let exec = Executor::new(state);
        let mut ctx = ExecutionContext::new(1, 0);
        exec.execute_transaction(&transfer_tx(from, addr(2), 0, 0, 21_000, 1), &mut ctx)
            .unwrap();
        exec.execute_transaction(&transfer_tx(from, addr(2), 1, 0, 21_000, 1), &mut ctx)
            .unwrap();
        assert_eq!(ctx.gas_used, 42_000);
    }

    #[test]
    fn block_gas_limit_exceeded() {
        let state = StateDB::new();
        let from = addr(1);
        state.set_account(from, funded(0, u64::MAX));
        let cfg = GasConfig {
            tx_gas_cost: 21_000,
            data_gas_cost: 16,
            max_gas_per_block: 21_000,
        };
        let exec = Executor::with_gas_config(state, cfg);
        let mut ctx = ExecutionContext::new(1, 0);
        ctx.gas_used = 1;
        let tx = transfer_tx(from, addr(2), 0, 0, 21_000, 1);
        assert_eq!(
            exec.execute_transaction(&tx, &mut ctx),
            Err(ExecutionError::BlockGasLimitExceeded)
        );
    }

    #[test]
    fn credits_missing_recipient() {
        let state = StateDB::new();
        state.set_account(addr(1), funded(0, 1_000_000));
        let exec = Executor::new(state);
        let to = addr(2);
        let mut ctx = ExecutionContext::new(1, 0);
        exec.execute_transaction(&transfer_tx(addr(1), to, 0, 7, 21_000, 1), &mut ctx)
            .unwrap();
        assert_eq!(
            exec.state().get_account(&to).unwrap().balance,
            U256::from(7u64)
        );
        assert_eq!(exec.state().get_account(&to).unwrap().nonce, 0);
    }

    #[test]
    fn outcome_receipt_fields() {
        let state = StateDB::new();
        state.set_account(addr(1), funded(0, 1_000_000));
        let exec = Executor::new(state);
        let tx = transfer_tx(addr(1), addr(2), 0, 1, 21_000, 1);
        let mut ctx = ExecutionContext::new(9, 123);
        let out = exec.execute_transaction(&tx, &mut ctx).unwrap();
        assert_eq!(out.receipt.gas_used, out.gas_used);
        assert!(out.call.logs.is_empty());
        assert_eq!(out.receipt.logs.len(), 0);
    }

    #[test]
    fn with_gas_config_custom_intrinsic() {
        let state = StateDB::new();
        state.set_account(addr(1), funded(0, 1_000_000));
        let cfg = GasConfig {
            tx_gas_cost: 10,
            data_gas_cost: 1,
            max_gas_per_block: 30_000_000,
        };
        let exec = Executor::with_gas_config(state, cfg);
        let tx = transfer_tx(addr(1), addr(2), 0, 0, 10, 1);
        let mut ctx = ExecutionContext::new(1, 0);
        let out = exec.execute_transaction(&tx, &mut ctx).unwrap();
        assert_eq!(out.gas_used, 10);
    }

    #[test]
    fn nonce_increments_across_txs() {
        let state = StateDB::new();
        state.set_account(addr(1), funded(0, 10_000_000));
        let exec = Executor::new(state);
        let mut ctx = ExecutionContext::new(1, 0);
        exec.execute_transaction(&transfer_tx(addr(1), addr(2), 0, 1, 21_000, 1), &mut ctx)
            .unwrap();
        exec.execute_transaction(&transfer_tx(addr(1), addr(2), 1, 1, 21_000, 1), &mut ctx)
            .unwrap();
        assert_eq!(exec.state().get_account(&addr(1)).unwrap().nonce, 2);
    }

    #[test]
    fn second_tx_wrong_nonce_fails() {
        let state = StateDB::new();
        state.set_account(addr(1), funded(0, 10_000_000));
        let exec = Executor::new(state);
        let mut ctx = ExecutionContext::new(1, 0);
        exec.execute_transaction(&transfer_tx(addr(1), addr(2), 0, 1, 21_000, 1), &mut ctx)
            .unwrap();
        assert!(matches!(
            exec.execute_transaction(&transfer_tx(addr(1), addr(2), 0, 1, 21_000, 1), &mut ctx),
            Err(ExecutionError::NonceMismatch { .. })
        ));
    }

    #[test]
    fn zero_value_call() {
        let state = StateDB::new();
        state.set_account(addr(1), funded(0, 1_000_000));
        let exec = Executor::new(state);
        let mut ctx = ExecutionContext::new(1, 0);
        exec.execute_transaction(&transfer_tx(addr(1), addr(2), 0, 0, 21_000, 1), &mut ctx)
            .unwrap();
        assert_eq!(
            exec.state().get_account(&addr(2)).unwrap().balance,
            U256::ZERO
        );
    }

    #[test]
    fn call_input_from_tx() {
        let tx = transfer_tx(addr(1), addr(2), 0, 5, 21_000, 1);
        let input = CallInput::from_tx(&tx);
        assert_eq!(input.kind, CallKind::Call);
        assert_eq!(input.to, Some(addr(2)));
        assert_eq!(input.value, U256::from(5u64));
    }

    #[test]
    fn execute_call_stub_succeeds() {
        let tx = create_tx(addr(1), 0, 0, 21_016);
        let result = execute_call(&CallInput::from_tx(&tx)).unwrap();
        assert!(result.success);
        assert!(result.logs.is_empty());
    }

    #[test]
    fn equal_balance_exact_cost() {
        let state = StateDB::new();
        // 21_000 gas * 1 price + 0 value
        state.set_account(addr(1), funded(0, 21_000));
        let exec = Executor::new(state);
        let mut ctx = ExecutionContext::new(1, 0);
        exec.execute_transaction(&transfer_tx(addr(1), addr(2), 0, 0, 21_000, 1), &mut ctx)
            .unwrap();
        assert_eq!(
            exec.state().get_account(&addr(1)).unwrap().balance,
            U256::ZERO
        );
    }

    #[test]
    fn one_wei_short_fails() {
        let state = StateDB::new();
        state.set_account(addr(1), funded(0, 20_999));
        let exec = Executor::new(state);
        let mut ctx = ExecutionContext::new(1, 0);
        assert_eq!(
            exec.execute_transaction(&transfer_tx(addr(1), addr(2), 0, 0, 21_000, 1), &mut ctx),
            Err(ExecutionError::InsufficientBalance)
        );
    }

    #[test]
    fn overflow_value_plus_gas() {
        let state = StateDB::new();
        let mut account = Account::new();
        account.balance = U256::MAX;
        state.set_account(addr(1), account);
        let exec = Executor::new(state);
        let tx = Transaction {
            from: addr(1),
            to: Some(addr(2)),
            value: U256::MAX,
            data: Bytes::new(),
            gas_price: U256::ONE,
            gas: 21_000,
            nonce: 0,
            signature: Signature::zero(),
            public_key: PublicKey::zero(),
        };
        let mut ctx = ExecutionContext::new(1, 0);
        assert_eq!(
            exec.execute_transaction(&tx, &mut ctx),
            Err(ExecutionError::Overflow)
        );
    }

    #[test]
    fn data_gas_refund_when_limit_higher() {
        let state = StateDB::new();
        state.set_account(addr(1), funded(0, 1_000_000));
        let exec = Executor::new(state);
        let mut tx = transfer_tx(addr(1), addr(2), 0, 0, 21_032, 1);
        tx.data = Bytes::from_slice(&[1, 2]); // +32 intrinsic
        let mut ctx = ExecutionContext::new(1, 0);
        let out = exec.execute_transaction(&tx, &mut ctx).unwrap();
        assert_eq!(out.gas_used, 21_032);
        assert_eq!(out.gas_refunded, 0);
    }

    #[test]
    fn context_default_beneficiary_zero() {
        let ctx = ExecutionContext::new(0, 0);
        assert!(ctx.beneficiary.is_zero());
        assert_eq!(ctx.gas_used, 0);
    }
}
