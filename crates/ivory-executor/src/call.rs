//! Call classification (VM comes in #7).

use ivory_core::{Log, Transaction};
use ivory_primitives::{Address, Bytes, U256};

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

/// Inputs to the (stub) call interpreter.
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

/// Result of a stubbed call (no WASM yet).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallResult {
    /// Execution succeeded.
    pub success: bool,
    /// Logs emitted (always empty until the VM).
    pub logs: Vec<Log>,
}

/// Stub interpreter: always succeeds, no storage, no logs.
///
/// Value transfer is applied by [`crate::Executor`] before this is called.
///
/// # Errors
///
/// Never fails in this stub; signature reserved for VM errors.
pub fn execute_call(_input: &CallInput) -> Result<CallResult, ExecutionError> {
    Ok(CallResult {
        success: true,
        logs: Vec::new(),
    })
}
