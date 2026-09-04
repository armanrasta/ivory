;; Example Ivory contract: mark storage slot 1.
(module
  (import "env" "storage_set" (func $set (param i32 i64)))
  (func (export "call")
    i32.const 1
    i64.const 1
    call $set
  )
)
