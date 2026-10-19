const values = [];
let next = 0;
let memory;
let encoder;
let decoder;

export function __rustier_js_init_memory(value) {
  memory = value;
}

function memoryIndex(value) {
  return typeof value === "bigint" ? Number(value) : value >>> 0;
}

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

export function __rustier_js_new_string(ptr, len) {
  decoder ??= new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
  const bytes = new Uint8Array(
    memory.buffer,
    memoryIndex(ptr),
    memoryIndex(len),
  );
  return __rustier_js_insert(decoder.decode(bytes));
}

export function __rustier_js_new_boolean(value) {
  return __rustier_js_insert(value !== 0);
}

export function __rustier_js_null() {
  return __rustier_js_insert(null);
}

export function __rustier_js_undefined() {
  return __rustier_js_insert(undefined);
}

export function __rustier_js_new_object() {
  return __rustier_js_insert({});
}

export function __rustier_js_type_of(handle) {
  const value = values[handle >>> 0];
  if (value === null) return 6;

  switch (typeof value) {
    case "string":
      return 0;
    case "number":
      return 1;
    case "bigint":
      return 2;
    case "boolean":
      return 3;
    case "symbol":
      return 4;
    case "undefined":
      return 5;
    case "object":
      return 7;
    case "function":
      return 8;
  }
}

export function __rustier_js_number(handle) {
  return values[handle >>> 0];
}

export function __rustier_js_boolean(handle) {
  return values[handle >>> 0];
}

export function __rustier_js_string_encode(handle) {
  encoder ??= new TextEncoder();
  return __rustier_js_insert(encoder.encode(values[handle >>> 0]));
}

export function __rustier_js_buffer_length(handle) {
  return values[handle >>> 0].length;
}

export function __rustier_js_buffer_copy(handle, ptr, len) {
  const bytes = new Uint8Array(
    memory.buffer,
    memoryIndex(ptr),
    memoryIndex(len),
  );
  bytes.set(values[handle >>> 0]);
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
