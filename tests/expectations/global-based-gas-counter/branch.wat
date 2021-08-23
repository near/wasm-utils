(module
  (type (;0;) (func (result i32)))
  (type (;1;) (func))
  (import "env" "out_of_gas_callback" (func (;0;) (type 1)))
  (func (;1;) (type 0) (result i32)
    (local i32 i32)
    global.get 0
    i64.const 13
    i64.lt_u
    if  ;; label = @1
      call 0
    end
    global.get 0
    i64.const 13
    i64.sub
    global.set 0
    block  ;; label = @1
      i32.const 0
      local.set 0
      i32.const 1
      local.set 1
      local.get 0
      local.get 1
      local.tee 0
      i32.add
      local.set 1
      i32.const 1
      br_if 0 (;@1;)
      global.get 0
      i64.const 5
      i64.lt_u
      if  ;; label = @2
        call 0
      end
      global.get 0
      i64.const 5
      i64.sub
      global.set 0
      local.get 0
      local.get 1
      local.tee 0
      i32.add
      local.set 1
    end
    local.get 1)
  (global (;0;) (mut i64) (i64.const 0))
  (export "remaining_ops" (global 0)))
