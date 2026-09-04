//! # Ivory Executor
//!
//! Transaction execution, gas metering, and value transfers. WASM via wasmi.

pub mod call;
pub mod context;
pub mod error;
pub mod executor;
pub mod gas;

pub use call::{CallInput, CallKind, CallResult, execute_call, output_from_i32};
pub use context::ExecutionContext;
pub use error::ExecutionError;
pub use executor::{ExecutionOutcome, Executor, SimulateOutcome, SimulateRequest};
pub use gas::{GasConfig, GasMeter, compute_intrinsic_gas, compute_intrinsic_gas_len};
