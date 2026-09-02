//! # Ivory Executor
//!
//! Transaction execution, gas metering, and value transfers. WASM via wasmi.

pub mod call;
pub mod context;
pub mod error;
pub mod executor;
pub mod gas;

pub use call::{CallInput, CallKind, CallResult, execute_call};
pub use context::ExecutionContext;
pub use error::ExecutionError;
pub use executor::{ExecutionOutcome, Executor};
pub use gas::{GasConfig, GasMeter, compute_intrinsic_gas};
