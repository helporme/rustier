const __rustier_handle_key = Symbol();
const __rustier_handle_finalizer = new FinalizationRegistry(__rustier_drop_handle);
let __rustier_is_handle;
let __rustier_handle_pointer;

class Rs {
  #pointer;

  constructor(pointer, key) {
    if (key !== __rustier_handle_key) {
      throw new TypeError('Rust handles can only be created by Rust');
    }

    this.#pointer = pointer;
    __rustier_handle_finalizer.register(this, pointer);
  }

  static {
    __rustier_is_handle = value =>
      typeof value === 'object' && value !== null && #pointer in value;
    __rustier_handle_pointer = value => value.#pointer;
  }
}
