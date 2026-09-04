//! Call classification and WASM dispatch.

use ivory_core::{Log, Transaction};
use ivory_primitives::{Address, Bytes, U256};
use ivory_state::StateDB;
use ivory_vm::WasmVm;

use crate::error::ExecutionError;

/// Kind of top-level call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallKind {
    /// Message call to an existing account.
    Call,
    /// Contract creation (`tx.to` is `None`).
    Create,
}

impl CallKind {
    /// Derive kind from a transaction.
    #[must_use]
    pub fn from_tx(tx: &Transaction) -> Self {
        if tx.is_create() {
            Self::Create
        } else {
            Self::Call
        }
    }
}

/// Inputs to the call interpreter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallInput {
    /// Call vs create.
    pub kind: CallKind,
    /// Sender.
    pub from: Address,
    /// Recipient (`None` for create).
    pub to: Option<Address>,
    /// Value already reserved from the sender.
    pub value: U256,
    /// Calldata or init code.
    pub data: Bytes,
}

impl CallInput {
    /// Build from a transaction.
    #[must_use]
    pub fn from_tx(tx: &Transaction) -> Self {
        Self {
            kind: CallKind::from_tx(tx),
            from: tx.from,
            to: tx.to,
            value: tx.value,
            data: tx.data.clone(),
        }
    }
}

/// Result of a call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallResult {
    /// Execution succeeded.
    pub success: bool,
    /// Logs emitted.
    pub logs: Vec<Log>,
    /// VM fuel consumed (0 for EOAs and CREATE).
    pub gas_used: u64,
}

/// Run the call: EOAs no-op, contracts via wasmi, CREATE installs no constructor.
///
/// Value transfer / CREATE account setup is applied by [`crate::Executor`] before this.
///
/// # Errors
///
/// [`ExecutionError::Vm`] when WASM is invalid, traps, or runs out of fuel.
pub fn execute_call(
    input: &CallInput,
    state: &StateDB,
    gas_limit: u64,
) -> Result<CallResult, ExecutionError> {
    match input.kind {
        CallKind::Create => Ok(CallResult {
            success: true,
            logs: Vec::new(),
            gas_used: 0,
        }),
        CallKind::Call => {
            let to = input.to.unwrap_or(input.from);
            let code = state.get_code(&to);
            if code.is_empty() {
                return Ok(CallResult {
                    success: true,
                    logs: Vec::new(),
                    gas_used: 0,
                });
            }
            let out = WasmVm::new()
                .execute(&code, state, to, gas_limit)
                .map_err(|e| ExecutionError::Vm(e.to_string()))?;
            let gas_used = gas_limit.saturating_sub(out.gas_left);
            Ok(CallResult {
                success: out.success,
                logs: out.logs,
                gas_used,
            })
        }
    }
}
