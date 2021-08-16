(module
  (type (;0;) (func))
  (import "env" "out_of_gas_callback" (func (;0;) (type 0)))
  (func (;1;) (type 0)
    global.get 0
    i32.const 2
    i32.lt_u
    if  ;; label = @1
      call 0
    end
    global.get 0
    i32.const 2
    i32.sub
    global.set 0
    i32.const 1
    if  ;; label = @1
      global.get 0
      i32.const 1
      i32.lt_u
      if  ;; label = @2
        call 0
      end
      global.get 0
      i32.const 1
      i32.sub
      global.set 0
      loop  ;; label = @2
        global.get 0
        i32.const 2
        i32.lt_u
        if  ;; label = @3
          call 0
        end
        global.get 0
        i32.const 2
        i32.sub
        global.set 0
        i32.const 123
        drop
      end
    end)
  (func (;2;) (type 0)
    global.get 0
    i32.const 1
    i32.lt_u
    if  ;; label = @1
      call 0
    end
    global.get 0
    i32.const 1
    i32.sub
    global.set 0
    block  ;; label = @1
    end)
  (global (;0;) (mut i32) (i32.const 0))
  (export "simple" (func 1))
  (export "remaining_gas" (global 0)))
