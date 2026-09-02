//! wasmi interpreter and `env` host stubs.

use ivory_primitives::{Address, H256, U256};
use ivory_state::StateDB;
use wasmi::{Caller, Engine, Linker, Module, Store};

use crate::error::VmError;

/// Result of a WASM invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VmOutput {
    /// Call returned without trap.
    pub success: bool,
    /// `i32` return of `call` if present.
    pub return_value: Option<i32>,
}

struct Host {
    state: StateDB,
    address: Address,
}

fn slot_key(slot: i32) -> H256 {
    let mut bytes = [0u8; 32];
    bytes[28..32].copy_from_slice(&slot.to_be_bytes());
    H256::from_bytes(bytes)
}

/// wasmi engine wrapper.
pub struct WasmVm {
    engine: Engine,
}

impl Default for WasmVm {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmVm {
    /// Default wasmi engine.
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
        }
    }

    /// Load `code`, instantiate with `env.storage_get` / `env.storage_set`, call `call`.
    ///
    /// `gas_limit` is accepted for the ABI and unused until metering lands.
    ///
    /// # Errors
    ///
    /// [`VmError`] on invalid modules, missing exports, or traps.
    pub fn execute(
        &self,
        code: &[u8],
        state: &StateDB,
        address: Address,
        _gas_limit: u64,
    ) -> Result<VmOutput, VmError> {
        let module =
            Module::new(&self.engine, code).map_err(|e| VmError::InvalidModule(e.to_string()))?;
        let mut store = Store::new(
            &self.engine,
            Host {
                state: state.clone(),
                address,
            },
        );
        let mut linker = Linker::new(&self.engine);

        linker
            .func_wrap(
                "env",
                "storage_get",
                |caller: Caller<'_, Host>, slot: i32| -> i64 {
                    let host = caller.data();
                    let val = host.state.get_storage(&host.address, &slot_key(slot));
                    val.0[0] as i64
                },
            )
            .map_err(|e| VmError::Instantiate(e.to_string()))?;

        linker
            .func_wrap(
                "env",
                "storage_set",
                |mut caller: Caller<'_, Host>, slot: i32, value: i64| {
                    let host = caller.data_mut();
                    host.state
                        .set_storage(host.address, slot_key(slot), U256::from(value as u64));
                },
            )
            .map_err(|e| VmError::Instantiate(e.to_string()))?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| VmError::Instantiate(e.to_string()))?
            .start(&mut store)
            .map_err(|e| VmError::Instantiate(e.to_string()))?;

        let Some(func) = instance.get_func(&store, "call") else {
            return Ok(VmOutput {
                success: true,
                return_value: None,
            });
        };

        if let Ok(typed) = func.typed::<(), i32>(&store) {
            return typed
                .call(&mut store, ())
                .map(|v| VmOutput {
                    success: true,
                    return_value: Some(v),
                })
                .map_err(|e| VmError::Trap(e.to_string()));
        }

        if let Ok(typed) = func.typed::<(), ()>(&store) {
            return typed
                .call(&mut store, ())
                .map(|()| VmOutput {
                    success: true,
                    return_value: None,
                })
                .map_err(|e| VmError::Trap(e.to_string()));
        }

        Err(VmError::Export(
            "call must have type () -> i32 or () -> ()".into(),
        ))
    }
}
