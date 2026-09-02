//! VM errors.

use thiserror::Error;

/// Failures from loading or running WASM.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum VmError {
    /// Bytes are not a valid WASM module.
    #[error("invalid wasm: {0}")]
    InvalidModule(String),
    /// Instantiation or start function failed.
    #[error("instantiate: {0}")]
    Instantiate(String),
    /// Exported function missing or wrong type.
    #[error("export: {0}")]
    Export(String),
    /// Trap during execution.
    #[error("trap: {0}")]
    Trap(String),
}
