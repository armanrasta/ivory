//! wasmi interpreter and `env` host functions.

use ivory_core::Log;
use ivory_primitives::{Address, Bytes, H256, U256};
use ivory_state::StateDB;
use wasmi::{Caller, Config, Engine, Linker, Module, Store};

use crate::error::VmError;

/// Result of a WASM invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VmOutput {
    /// Call returned without trap.
    pub success: bool,
    /// `i32` return of `call` if present.
    pub return_value: Option<i32>,
    /// Logs from `env.emit_log`.
    pub logs: Vec<Log>,
    /// Remaining fuel (equals `gas_limit` minus consumed).
    pub gas_left: u64,
}

struct Host {
    state: StateDB,
    address: Address,
    logs: Vec<Log>,
    calldata: Vec<u8>,
}

fn slot_key(slot: i32) -> H256 {
    let mut bytes = [0u8; 32];
    bytes[28..32].copy_from_slice(&slot.to_be_bytes());
    H256::from_bytes(bytes)
}

fn topic_from_i32(topic: i32) -> H256 {
    let mut bytes = [0u8; 32];
    bytes[28..32].copy_from_slice(&topic.to_be_bytes());
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
    /// Default wasmi engine with fuel metering enabled.
    #[must_use]
    pub fn new() -> Self {
        let mut config = Config::default();
        config.consume_fuel(true);
        Self {
            engine: Engine::new(&config),
        }
    }

    /// Load `code`, instantiate with host imports, call `call`.
    ///
    /// `gas_limit` is remaining gas after intrinsic. `0` is treated as a large
    /// default so unit tests that omit metering still run.
    ///
    /// # Errors
    ///
    /// [`VmError`] on invalid modules, missing exports, traps, or out-of-fuel.
    pub fn execute(
        &self,
        code: &[u8],
        state: &StateDB,
        address: Address,
        gas_limit: u64,
        input: &[u8],
    ) -> Result<VmOutput, VmError> {
        let fuel = if gas_limit == 0 {
            10_000_000
        } else {
            gas_limit
        };
        let module =
            Module::new(&self.engine, code).map_err(|e| VmError::InvalidModule(e.to_string()))?;
        let mut store = Store::new(
            &self.engine,
            Host {
                state: state.clone(),
                address,
                logs: Vec::new(),
                calldata: input.to_vec(),
            },
        );
        store
            .set_fuel(fuel)
            .map_err(|e| VmError::Instantiate(e.to_string()))?;
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

        linker
            .func_wrap(
                "env",
                "emit_log",
                |mut caller: Caller<'_, Host>, topic: i32| {
                    let host = caller.data_mut();
                    let addr = host.address;
                    host.logs.push(Log {
                        address: addr,
                        topics: vec![topic_from_i32(topic)],
                        data: Bytes::new(),
                    });
                },
            )
            .map_err(|e| VmError::Instantiate(e.to_string()))?;

        linker
            .func_wrap("env", "calldata_len", |caller: Caller<'_, Host>| -> i32 {
                caller.data().calldata.len() as i32
            })
            .map_err(|e| VmError::Instantiate(e.to_string()))?;

        linker
            .func_wrap(
                "env",
                "calldata_at",
                |caller: Caller<'_, Host>, index: i32| -> i32 {
                    caller
                        .data()
                        .calldata
                        .get(index as usize)
                        .copied()
                        .map(i32::from)
                        .unwrap_or(0)
                },
            )
            .map_err(|e| VmError::Instantiate(e.to_string()))?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| VmError::Instantiate(e.to_string()))?
            .start(&mut store)
            .map_err(|e| VmError::Instantiate(e.to_string()))?;

        let Some(func) = instance.get_func(&store, "call") else {
            let gas_left = remaining_fuel(&store, gas_limit);
            return Ok(VmOutput {
                success: true,
                return_value: None,
                logs: store.into_data().logs,
                gas_left,
            });
        };

        let call_result = if let Ok(typed) = func.typed::<(), i32>(&store) {
            typed.call(&mut store, ()).map(Some)
        } else if let Ok(typed) = func.typed::<(), ()>(&store) {
            typed.call(&mut store, ()).map(|()| None)
        } else {
            return Err(VmError::Export(
                "call must have type () -> i32 or () -> ()".into(),
            ));
        };

        match call_result {
            Ok(return_value) => {
                let gas_left = remaining_fuel(&store, gas_limit);
                Ok(VmOutput {
                    success: true,
                    return_value,
                    logs: store.into_data().logs,
                    gas_left,
                })
            }
            Err(e) => Err(VmError::Trap(e.to_string())),
        }
    }
}

fn remaining_fuel(store: &Store<Host>, gas_limit: u64) -> u64 {
    if gas_limit == 0 {
        0
    } else {
        store.get_fuel().unwrap_or(0)
    }
}
