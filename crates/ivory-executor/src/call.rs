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
}

/// Run the call: EOAs no-op, contracts via wasmi, CREATE still stubbed (#16).
///
/// Value transfer is applied by [`crate::Executor`] before this is called.
///
/// # Errors
///
/// [`ExecutionError::Vm`] when WASM is invalid or traps.
pub fn execute_call(input: &CallInput, state: &StateDB) -> Result<CallResult, ExecutionError> {
    match input.kind {
        CallKind::Create => Ok(CallResult {
            success: true,
            logs: Vec::new(),
        }),
        CallKind::Call => {
            let to = input.to.unwrap_or(input.from);
            let code = state.get_code(&to);
            if code.is_empty() {
                return Ok(CallResult {
                    success: true,
                    logs: Vec::new(),
                });
            }
            let out = WasmVm::new()
                .execute(&code, state, to, 0)
                .map_err(|e| ExecutionError::Vm(e.to_string()))?;
            Ok(CallResult {
                success: out.success,
                logs: Vec::new(),
            })
        }
    }
}
