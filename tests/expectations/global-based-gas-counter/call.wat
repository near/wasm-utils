(module
  (type (;0;) (func (param i32 i32) (result i32)))
  (type (;1;) (func (param i64)))
  (import "env" "out_of_gas_callback" (func (;0;) (type 1)))
  (func (;1;) (type 0) (param i32 i32) (result i32)
    (local i32)
    global.get 0
    i64.const 5
    i64.lt_u
    if  ;; label = @1
      i64.const 5
      call 0
    end
    global.get 0
    i64.const 5
    i64.sub
    global.set 0
    local.get 0
    local.get 1
    call 2
    local.set 2
    local.get 2)
  (func (;2;) (type 0) (param i32 i32) (result i32)
    global.get 0
    i64.const 3
    i64.lt_u
    if  ;; label = @1
      i64.const 3
      call 0
    end
    global.get 0
    i64.const 3
    i64.sub
    global.set 0
    local.get 0
    local.get 1
    i32.add)
  (global (;0;) (mut i64) (i64.const 0))
  (export "remaining_ops" (global 0)))
