//! Core error types.

use thiserror::Error;

/// Errors from block validation.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum BlockError {
    /// `gas_used` exceeds `gas_limit`.
    #[error("gas used exceeds limit")]
    GasExceeded,
    /// Parent hash does not match the expected parent.
    #[error("invalid parent hash")]
    InvalidParentHash,
    /// Block timestamp is invalid relative to its parent.
    #[error("invalid timestamp")]
    InvalidTimestamp,
    /// Quant envelope in `tx.data` failed structural validation.
    #[error("invalid quant envelope: {0}")]
    InvalidQuantEnvelope(&'static str),
    /// Header `transactions_root` does not match `list_root(transactions)`.
    #[error("invalid transactions root")]
    InvalidTransactionsRoot,
    /// Header `receipts_root` does not match `list_root(receipts)`.
    #[error("invalid receipts root")]
    InvalidReceiptsRoot,
    /// Transaction and receipt lists have different lengths.
    #[error("transaction/receipt count mismatch")]
    TxReceiptCountMismatch,
}
