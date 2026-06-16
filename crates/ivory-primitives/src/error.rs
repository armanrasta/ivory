// crates/ivory-primitives/src/error.rs

use thiserror::Error;

/// Errors related to primitive types.
#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum PrimitiveError {
    /// Integer overflow
    #[error("Integer overflow")]
    Overflow,

    /// Invalid hex string
    #[error("Invalid hex string")]
    InvalidHex,

    /// Invalid length for fixed-size types
    #[error("Invalid length: expected {expected}, got {got}")]
    InvalidLength {
        /// Expected length
        expected: usize,
        /// Actual length
        got: usize,
    },
}
