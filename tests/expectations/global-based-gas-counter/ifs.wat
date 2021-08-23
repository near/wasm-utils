(module
  (type (;0;) (func (param i32) (result i32)))
  (type (;1;) (func))
  (import "env" "out_of_gas_callback" (func (;0;) (type 1)))
  (func (;1;) (type 0) (param i32) (result i32)
    global.get 0
    i64.const 2
    i64.lt_u
    if  ;; label = @1
      call 0
    end
    global.get 0
    i64.const 2
    i64.sub
    global.set 0
    i32.const 1
    if (result i32)  ;; label = @1
      global.get 0
      i64.const 3
      i64.lt_u
      if  ;; label = @2
        call 0
      end
      global.get 0
      i64.const 3
      i64.sub
      global.set 0
      local.get 0
      i32.const 1
      i32.add
    else
      global.get 0
      i64.const 2
      i64.lt_u
      if  ;; label = @2
        call 0
      end
      global.get 0
      i64.const 2
      i64.sub
      global.set 0
      local.get 0
      i32.popcnt
    end)
  (global (;0;) (mut i64) (i64.const 0))
  (export "remaining_ops" (global 0)))
