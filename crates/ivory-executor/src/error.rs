//! Execution errors.

use thiserror::Error;

/// Errors from transaction execution.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    /// Account nonce does not match the transaction.
    #[error("nonce mismatch: expected {expected}, got {got}")]
    NonceMismatch {
        /// Account nonce.
        expected: u64,
        /// Transaction nonce.
        got: u64,
    },
    /// Sender cannot cover `value + gas * gas_price`.
    #[error("insufficient balance")]
    InsufficientBalance,
    /// Intrinsic gas exceeds the transaction gas limit.
    #[error("out of gas")]
    OutOfGas,
    /// Checked `U256` arithmetic overflowed.
    #[error("integer overflow")]
    Overflow,
    /// Cumulative block gas would exceed [`crate::GasConfig::max_gas_per_block`].
    #[error("block gas limit exceeded")]
    BlockGasLimitExceeded,
    /// WASM load or execution failed.
    #[error("vm: {0}")]
    Vm(String),
}
