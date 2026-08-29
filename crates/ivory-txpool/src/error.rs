//! Transaction pool errors.

use thiserror::Error;

/// Errors from mempool admission.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TxPoolError {
    /// Sender nonce is below the next expected value.
    #[error("nonce too low: expected {expected}, got {got}")]
    NonceTooLow {
        /// Next nonce the pool will accept.
        expected: u64,
        /// Nonce on the submitted transaction.
        got: u64,
    },
    /// Sender nonce skips ahead of the next expected value.
    #[error("nonce gap: expected {expected}, got {got}")]
    NonceGap {
        /// Next nonce the pool will accept.
        expected: u64,
        /// Nonce on the submitted transaction.
        got: u64,
    },
    /// Identical transaction hash is already pending.
    #[error("transaction already known")]
    AlreadyKnown,
    /// Transaction gas limit is below [`crate::PoolConfig::min_gas`].
    #[error("gas limit {got} below minimum {min}")]
    GasLimitTooLow {
        /// Configured minimum gas.
        min: u64,
        /// Gas limit on the transaction.
        got: u64,
    },
    /// `max_pending` transactions are already queued.
    #[error("pool is full")]
    PoolFull,
    /// Sender already has `max_per_sender` pending transactions.
    #[error("sender pending limit reached")]
    SenderLimitReached,
}
