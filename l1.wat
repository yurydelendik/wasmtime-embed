(module
  (import "test" "callback" (func $callback (param i32)))
  (import "gcd" "gcd" (func $gcd (param i32 i32) (result i32)))
  (func $main
    i32.const 6
    i32.const 27
    call $gcd
    call $callback
  )
  (export "main" (func $main))
)
