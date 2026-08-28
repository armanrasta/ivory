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
}
