const values = [];
let next = 0;

export function __rustier_js_insert(value) {
  if (next === values.length) {
    values.push(next + 1);
  }

  const index = next;
  next = values[index];
  values[index] = value;
  return index;
}

export function __rustier_js_new_number(value) {
  return __rustier_js_insert(value);
}

export function __rustier_js_new_object() {
  return __rustier_js_insert({});
}

export function __rustier_js_clone_ref(handle) {
  return __rustier_js_insert(values[handle >>> 0]);
}

export function __rustier_js_take(handle) {
  const value = values[handle >>> 0];
  __rustier_js_drop_ref(handle);
  return value;
}

export function __rustier_js_drop_ref(handle) {
  handle >>>= 0;
  values[handle] = next;
  next = handle;
}
