//! Transaction executor.

use ivory_core::{Receipt, Transaction};
use ivory_primitives::{Address, Bytes, U256, keccak256};
use ivory_state::StateDB;

use crate::call::{CallInput, CallKind, CallResult, execute_call};
use crate::context::ExecutionContext;
use crate::error::ExecutionError;
use crate::gas::{GasConfig, GasMeter, compute_intrinsic_gas, compute_intrinsic_gas_len};

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
    /// Contract creation (`to: None`) derives `Address::create(from, nonce)`, credits
    /// the endowment, and installs `tx.data` as runtime bytecode (no constructor).
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
        let mut meter = GasMeter::new(tx.gas, intrinsic)?;

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

        if let Some(to) = tx.to {
            let mut recipient = self.state.get_account(&to).unwrap_or_default();
            recipient.balance = recipient
                .balance
                .checked_add(tx.value)
                .ok_or(ExecutionError::Overflow)?;
            self.state.set_account(to, recipient);
        } else {
            let created = Address::create(&tx.from, tx.nonce);
            let mut account = self.state.get_account(&created).unwrap_or_default();
            account.balance = account
                .balance
                .checked_add(tx.value)
                .ok_or(ExecutionError::Overflow)?;
            let code = tx.data.clone();
            account.code_hash = keccak256(code.as_slice());
            self.state.set_account(created, account);
            self.state.set_code(created, code);
        }

        let input = CallInput::from_tx(tx);
        let call = execute_call(&input, &self.state, meter.remaining)?;
        if call.gas_used > 0 {
            meter.spend(call.gas_used)?;
        }

        let gas_used = meter.gas_used();
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

    /// Simulate a call or CREATE on `self.state` (caller should [`StateDB::fork`]).
    ///
    /// Ignores nonce and `gas * gas_price`. Transfers `value` when the sender
    /// can cover it. WASM `data` is unused by the VM (protocol `call` has no
    /// calldata).
    ///
    /// # Errors
    ///
    /// [`ExecutionError`] on balance, gas, overflow, or VM failures.
    pub fn simulate(&self, req: SimulateRequest) -> Result<SimulateOutcome, ExecutionError> {
        let gas = if req.gas == 0 { 10_000_000 } else { req.gas };
        let intrinsic = compute_intrinsic_gas_len(req.data.len(), &self.gas_config);
        let mut meter = GasMeter::new(gas, intrinsic)?;

        let sender = self.state.get_account(&req.from).unwrap_or_default();
        if sender.balance < req.value {
            return Err(ExecutionError::InsufficientBalance);
        }

        if !req.value.is_zero() {
            let mut from_acc = sender.clone();
            from_acc.balance = from_acc
                .balance
                .checked_sub(req.value)
                .ok_or(ExecutionError::Overflow)?;
            self.state.set_account(req.from, from_acc);
        }

        if let Some(to) = req.to {
            if !req.value.is_zero() {
                let mut recipient = self.state.get_account(&to).unwrap_or_default();
                recipient.balance = recipient
                    .balance
                    .checked_add(req.value)
                    .ok_or(ExecutionError::Overflow)?;
                self.state.set_account(to, recipient);
            }
        } else {
            let created = Address::create(&req.from, sender.nonce);
            let mut account = self.state.get_account(&created).unwrap_or_default();
            if !req.value.is_zero() {
                account.balance = account
                    .balance
                    .checked_add(req.value)
                    .ok_or(ExecutionError::Overflow)?;
            }
            let code = req.data.clone();
            account.code_hash = keccak256(code.as_slice());
            self.state.set_account(created, account);
            self.state.set_code(created, code);
        }

        let input = CallInput {
            kind: if req.to.is_none() {
                CallKind::Create
            } else {
                CallKind::Call
            },
            from: req.from,
            to: req.to,
            value: req.value,
            data: req.data,
        };
        let call = execute_call(&input, &self.state, meter.remaining)?;
        if call.gas_used > 0 {
            meter.spend(call.gas_used)?;
        }

        Ok(SimulateOutcome {
            call,
            gas_used: meter.gas_used(),
            intrinsic,
        })
    }
}

/// RPC / `eth_call` inputs. Nonce and gas price are ignored.
#[derive(Clone, Debug)]
pub struct SimulateRequest {
    /// Sender (`0x0` if omitted by the client).
    pub from: Address,
    /// Recipient (`None` is CREATE).
    pub to: Option<Address>,
    /// Value to transfer on the fork.
    pub value: U256,
    /// Calldata or CREATE init code (WASM `call` ignores calldata).
    pub data: Bytes,
    /// Gas limit (`0` means 10_000_000).
    pub gas: u64,
}

impl Default for SimulateRequest {
    fn default() -> Self {
        Self {
            from: Address::ZERO,
            to: None,
            value: U256::ZERO,
            data: Bytes::new(),
            gas: 10_000_000,
        }
    }
}

/// Result of [`Executor::simulate`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulateOutcome {
    /// Call interpreter result (including encoded output).
    pub call: CallResult,
    /// Intrinsic plus VM fuel (`eth_estimateGas`).
    pub gas_used: u64,
    /// Intrinsic component of [`Self::gas_used`].
    pub intrinsic: u64,
}

#[cfg(test)]
mod tests {
    use ivory_core::Account;
    use ivory_primitives::{Address, Bytes, H256, PublicKey, Signature, U256};

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
    fn create_path_credits_created_account() {
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
        assert_eq!(sender.balance, U256::from(1_000_000u64 - 21_016 - 100));
        let created = Address::create(&from, 0);
        let acc = exec.state().get_account(&created).unwrap();
        assert_eq!(acc.balance, U256::from(100u64));
        assert_eq!(exec.state().get_code(&created), vec![0x00]);
        assert_ne!(acc.code_hash, ivory_primitives::H256::ZERO);
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
        let result = execute_call(&CallInput::from_tx(&tx), &StateDB::new(), 0).unwrap();
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

    #[test]
    fn wasm_contract_call_sets_storage() {
        let wasm = wat::parse_str(
            r#"(module
              (import "env" "storage_set" (func $set (param i32 i64)))
              (func (export "call")
                i32.const 2
                i64.const 55
                call $set
              )
            )"#,
        )
        .unwrap();
        let state = StateDB::new();
        state.set_account(addr(1), funded(0, 1_000_000));
        state.set_code(addr(2), Bytes::from_vec(wasm));
        let exec = Executor::new(state);
        let mut ctx = ExecutionContext::new(1, 0);
        let out = exec
            .execute_transaction(&transfer_tx(addr(1), addr(2), 0, 0, 100_000, 1), &mut ctx)
            .unwrap();
        assert!(out.receipt.status);
        let mut key = [0u8; 32];
        key[31] = 2;
        assert_eq!(
            exec.state().get_storage(&addr(2), &H256::from_bytes(key)),
            U256::from(55u64)
        );
    }

    #[test]
    fn invalid_contract_wasm_is_vm_error() {
        let state = StateDB::new();
        state.set_account(addr(1), funded(0, 1_000_000));
        state.set_code(addr(2), Bytes::from_slice(&[0x00, 0x01, 0x02]));
        let exec = Executor::new(state);
        let mut ctx = ExecutionContext::new(1, 0);
        assert!(matches!(
            exec.execute_transaction(&transfer_tx(addr(1), addr(2), 0, 0, 21_000, 1), &mut ctx),
            Err(ExecutionError::Vm(_))
        ));
    }

    #[test]
    fn simulate_transfer_does_not_touch_live() {
        let live = StateDB::new();
        let from = addr(1);
        let to = addr(2);
        live.set_account(from, funded(0, 1_000_000));
        let fork = live.fork();
        let exec = Executor::new(fork);
        let out = exec
            .simulate(SimulateRequest {
                from,
                to: Some(to),
                value: U256::from(100u64),
                data: Bytes::new(),
                gas: 21_000,
            })
            .unwrap();
        assert!(out.call.success);
        assert!(out.call.output.as_slice().is_empty());
        assert_eq!(out.gas_used, 21_000);
        assert_eq!(
            live.get_account(&from).unwrap().balance,
            U256::from(1_000_000u64)
        );
        assert!(live.get_account(&to).is_none());
        assert_eq!(
            exec.state().get_account(&to).unwrap().balance,
            U256::from(100u64)
        );
        assert_eq!(exec.state().get_account(&from).unwrap().nonce, 0);
    }

    #[test]
    fn simulate_wasm_returns_padded_i32() {
        let wasm = wat::parse_str(
            r#"(module
              (func (export "call") (result i32)
                i32.const 42
              )
            )"#,
        )
        .unwrap();
        let live = StateDB::new();
        live.set_code(addr(2), Bytes::from_vec(wasm));
        let before = live.get_storage(&addr(2), &H256::ZERO);
        let exec = Executor::new(live.fork());
        let out = exec
            .simulate(SimulateRequest {
                from: addr(1),
                to: Some(addr(2)),
                value: U256::ZERO,
                data: Bytes::new(),
                gas: 100_000,
            })
            .unwrap();
        assert_eq!(out.call.output, crate::output_from_i32(42));
        assert_eq!(live.get_storage(&addr(2), &H256::ZERO), before);
        assert!(out.gas_used >= 21_000);
    }

    #[test]
    fn simulate_insufficient_value() {
        let state = StateDB::new();
        state.set_account(addr(1), funded(0, 10));
        let exec = Executor::new(state);
        assert_eq!(
            exec.simulate(SimulateRequest {
                from: addr(1),
                to: Some(addr(2)),
                value: U256::from(11u64),
                data: Bytes::new(),
                gas: 21_000,
            }),
            Err(ExecutionError::InsufficientBalance)
        );
    }
}
