const __rustier_handle_key = Symbol();
const __rustier_handle_finalizer = new FinalizationRegistry(__rustier_drop_handle);
const __rustier_handles = new WeakSet();
const __rustier_has_handle = __rustier_handles.has.bind(__rustier_handles);
const __rustier_register_handle = __rustier_handles.add.bind(__rustier_handles);
const __rustier_define_pointer = Object.defineProperty;
const __rustier_pointer_bigint = BigInt;

function __rustier_new_handle(pointer) {
  return new Rs(__rustier_pointer_bigint(pointer), __rustier_handle_key);
}

class Rs {
  constructor(pointer, key) {
    if (key !== __rustier_handle_key) {
      throw new TypeError('Rust handles can only be created by Rust');
    }

    __rustier_define_pointer(this, ' __UNSAFE_ptr', { __proto__: null, value: pointer });
    __rustier_register_handle(this);
    __rustier_handle_finalizer.register(this, pointer);
  }
}
