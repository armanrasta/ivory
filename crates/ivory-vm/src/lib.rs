//! # Ivory VM
//!
//! wasmi interpreter with `env.storage_get` / `env.storage_set` / `env.emit_log`.

pub mod error;
pub mod vm;

pub use error::VmError;
pub use vm::{VmOutput, WasmVm};

#[cfg(test)]
mod tests {
    use ivory_primitives::{Address, U256};
    use ivory_state::StateDB;

    use super::*;

    fn addr() -> Address {
        Address::from_bytes([7u8; 20])
    }

    fn wat_to_wasm(wat: &str) -> Vec<u8> {
        wat::parse_str(wat).expect("wat")
    }

    #[test]
    fn default_matches_new() {
        let _ = WasmVm::default();
    }

    #[test]
    fn call_returns_42() {
        let wasm = wat_to_wasm(
            r#"(module
              (func (export "call") (result i32)
                i32.const 42
              )
            )"#,
        );
        let out = WasmVm::new()
            .execute(&wasm, &StateDB::new(), addr(), 0)
            .unwrap();
        assert!(out.success);
        assert_eq!(out.return_value, Some(42));
    }

    #[test]
    fn invalid_wasm_errors() {
        let err = WasmVm::new()
            .execute(&[0x00, 0x01, 0x02], &StateDB::new(), addr(), 0)
            .unwrap_err();
        assert!(matches!(err, VmError::InvalidModule(_)));
    }

    #[test]
    fn empty_bytes_invalid() {
        assert!(matches!(
            WasmVm::new().execute(&[], &StateDB::new(), addr(), 0),
            Err(VmError::InvalidModule(_))
        ));
    }

    #[test]
    fn missing_call_export_succeeds() {
        let wasm = wat_to_wasm(r#"(module (func (export "other") (result i32) i32.const 1))"#);
        let out = WasmVm::new()
            .execute(&wasm, &StateDB::new(), addr(), 0)
            .unwrap();
        assert!(out.success);
        assert_eq!(out.return_value, None);
    }

    #[test]
    fn call_void_export() {
        let wasm = wat_to_wasm(r#"(module (func (export "call")))"#);
        let out = WasmVm::new()
            .execute(&wasm, &StateDB::new(), addr(), 0)
            .unwrap();
        assert!(out.success);
        assert_eq!(out.return_value, None);
    }

    #[test]
    fn trap_unreachable() {
        let wasm = wat_to_wasm(
            r#"(module
              (func (export "call") (result i32)
                unreachable
              )
            )"#,
        );
        assert!(matches!(
            WasmVm::new().execute(&wasm, &StateDB::new(), addr(), 0),
            Err(VmError::Trap(_))
        ));
    }

    #[test]
    fn storage_get_default_zero() {
        let wasm = wat_to_wasm(
            r#"(module
              (import "env" "storage_get" (func $get (param i32) (result i64)))
              (func (export "call") (result i32)
                i32.const 3
                call $get
                i32.wrap_i64
              )
            )"#,
        );
        let out = WasmVm::new()
            .execute(&wasm, &StateDB::new(), addr(), 0)
            .unwrap();
        assert_eq!(out.return_value, Some(0));
    }

    #[test]
    fn storage_set_then_get() {
        let wasm = wat_to_wasm(
            r#"(module
              (import "env" "storage_get" (func $get (param i32) (result i64)))
              (import "env" "storage_set" (func $set (param i32 i64)))
              (func (export "call") (result i32)
                i32.const 1
                i64.const 99
                call $set
                i32.const 1
                call $get
                i32.wrap_i64
              )
            )"#,
        );
        let state = StateDB::new();
        let out = WasmVm::new().execute(&wasm, &state, addr(), 0).unwrap();
        assert_eq!(out.return_value, Some(99));
        let mut key = [0u8; 32];
        key[31] = 1;
        assert_eq!(
            state.get_storage(&addr(), &ivory_primitives::H256::from_bytes(key)),
            U256::from(99u64)
        );
    }

    #[test]
    fn storage_isolated_by_address() {
        let wasm = wat_to_wasm(
            r#"(module
              (import "env" "storage_set" (func $set (param i32 i64)))
              (func (export "call")
                i32.const 0
                i64.const 7
                call $set
              )
            )"#,
        );
        let state = StateDB::new();
        WasmVm::new().execute(&wasm, &state, addr(), 0).unwrap();
        let other = Address::from_bytes([8u8; 20]);
        assert_eq!(
            state.get_storage(&other, &ivory_primitives::H256::ZERO),
            U256::ZERO
        );
        assert_eq!(
            state.get_storage(&addr(), &ivory_primitives::H256::ZERO),
            U256::from(7u64)
        );
    }

    #[test]
    fn wrong_call_type() {
        let wasm = wat_to_wasm(
            r#"(module
              (func (export "call") (param i32) (result i32)
                local.get 0
              )
            )"#,
        );
        assert!(matches!(
            WasmVm::new().execute(&wasm, &StateDB::new(), addr(), 0),
            Err(VmError::Export(_))
        ));
    }

    #[test]
    fn missing_host_import_fails_instantiate() {
        let wasm = wat_to_wasm(
            r#"(module
              (import "env" "nope" (func $n (result i32)))
              (func (export "call") (result i32)
                call $n
              )
            )"#,
        );
        assert!(matches!(
            WasmVm::new().execute(&wasm, &StateDB::new(), addr(), 0),
            Err(VmError::Instantiate(_))
        ));
    }

    #[test]
    fn emit_log_records_topic() {
        let wasm = wat_to_wasm(
            r#"(module
              (import "env" "emit_log" (func $log (param i32)))
              (func (export "call")
                i32.const 7
                call $log
              )
            )"#,
        );
        let out = WasmVm::new()
            .execute(&wasm, &StateDB::new(), addr(), 50_000)
            .unwrap();
        assert_eq!(out.logs.len(), 1);
        assert_eq!(out.logs[0].address, addr());
        let mut topic = [0u8; 32];
        topic[31] = 7;
        assert_eq!(
            out.logs[0].topics[0],
            ivory_primitives::H256::from_bytes(topic)
        );
        assert!(out.gas_left < 50_000);
    }
}
