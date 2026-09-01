//! Consensus errors.

use thiserror::Error;

/// Errors from Proof-of-Authority header validation and sealing.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConsensusError {
    /// Miner is not in the validator set.
    #[error("not a validator")]
    NotValidator,
    /// `extra_data` does not contain `required_signatures` seals.
    #[error("insufficient seals: expected {expected}, got {got}")]
    InsufficientSeals {
        /// Required number of 64-byte seals.
        expected: usize,
        /// Seals present in `extra_data`.
        got: usize,
    },
    /// Block timestamp is earlier than the parent.
    #[error("invalid timestamp")]
    InvalidTimestamp,
    /// Seal bytes are the wrong length or otherwise malformed.
    #[error("invalid seal")]
    InvalidSeal,
    /// No validators configured.
    #[error("empty validator set")]
    EmptyValidatorSet,
}
