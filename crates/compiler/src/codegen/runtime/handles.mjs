const __rustier_handle_key = Symbol();
const __rustier_handle_finalizer = new FinalizationRegistry(__rustier_drop_handle);
const __rustier_handles = new WeakSet();

function __rustier_new_handle(pointer) {
  return new Rs(BigInt(pointer), __rustier_handle_key);
}

class Rs {
  constructor(pointer, key) {
    if (key !== __rustier_handle_key) {
      throw new TypeError('Rust handles can only be created by Rust');
    }

    Object.defineProperty(this, ' __UNSAFE_ptr', { value: pointer });
    __rustier_handles.add(this);
    __rustier_handle_finalizer.register(this, pointer);
  }
}
