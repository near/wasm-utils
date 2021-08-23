(module
  (type (;0;) (func (param i32 i32)))
  (type (;1;) (func))
  (import "env" "ext_return" (func (;0;) (type 0)))
  (import "env" "memory" (memory (;0;) 1 1))
  (import "env" "out_of_gas_callback" (func (;1;) (type 1)))
  (func (;2;) (type 1)
    global.get 0
    i64.const 4
    i64.lt_u
    if  ;; label = @1
      call 1
    end
    global.get 0
    i64.const 4
    i64.sub
    global.set 0
    i32.const 8
    i32.const 4
    call 0
    unreachable)
  (func (;3;) (type 1))
  (global (;0;) (mut i64) (i64.const 0))
  (export "call" (func 3))
  (export "remaining_ops" (global 0))
  (start 2)
  (data (;0;) (i32.const 8) "\01\02\03\04"))
