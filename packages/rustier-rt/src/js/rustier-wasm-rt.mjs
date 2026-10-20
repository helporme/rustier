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

function stringFromMemory(ptr, len) {
  decoder ??= new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
  const bytes = new Uint8Array(
    memory.buffer,
    memoryIndex(ptr),
    memoryIndex(len),
  );
  return decoder.decode(bytes);
}

export function __rustier_js_new_string(ptr, len) {
  return __rustier_js_insert(stringFromMemory(ptr, len));
}

export function __rustier_js_new_symbol(ptr, len, hasDescription) {
  const description = hasDescription ? stringFromMemory(ptr, len) : undefined;
  return __rustier_js_insert(Symbol(description));
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

function valueResult(failed, result, error) {
  const rooted = __rustier_js_insert(result);
  new DataView(memory.buffer).setUint32(memoryIndex(failed), error, true);
  return rooted;
}

function argumentsFromMemory(ptr, len) {
  const length = memoryIndex(len);
  const handles = new Uint32Array(memory.buffer, memoryIndex(ptr), length);
  const args = new Array(length);
  for (let index = 0; index < length; index++) {
    args[index] = values[handles[index]];
  }
  return args;
}

export function __rustier_js_strict_eq(left, right) {
  return values[left >>> 0] === values[right >>> 0];
}

export function __rustier_js_soft_eq(left, right) {
  const a = values[left >>> 0];
  const b = values[right >>> 0];
  return a === b ? a !== 0 || 1 / a === 1 / b : a !== a && b !== b;
}

export function __rustier_js_get(handle, key, failed) {
  let result;
  let error = 0;
  try {
    result = Reflect.get(values[handle >>> 0], values[key >>> 0]);
  } catch (exception) {
    result = exception;
    error = 1;
  }
  return valueResult(failed, result, error);
}

function reflectBoolean(operation, handle, key, value, failed) {
  let result;
  let error = 0;
  try {
    const accepted = Reflect[operation](
      values[handle >>> 0],
      values[key >>> 0],
      values[value >>> 0],
    );
    if (typeof accepted !== "boolean") {
      throw new TypeError(`Reflect.${operation} did not return a boolean`);
    }
    result = accepted ? 1 : 0;
  } catch (exception) {
    result = __rustier_js_insert(exception);
    error = 1;
  }
  new DataView(memory.buffer).setUint32(memoryIndex(failed), error, true);
  return result;
}

export function __rustier_js_set(handle, key, value, failed) {
  return reflectBoolean("set", handle, key, value, failed);
}

export function __rustier_js_has(handle, key, failed) {
  return reflectBoolean("has", handle, key, 0, failed);
}

export function __rustier_js_delete(handle, key, failed) {
  return reflectBoolean("deleteProperty", handle, key, 0, failed);
}

export function __rustier_js_define_property(handle, key, descriptor, failed) {
  return reflectBoolean("defineProperty", handle, key, descriptor, failed);
}

export function __rustier_js_get_own_property_descriptor(handle, key, failed) {
  let result;
  let error = 0;
  try {
    result = Reflect.getOwnPropertyDescriptor(
      values[handle >>> 0],
      values[key >>> 0],
    );
  } catch (exception) {
    result = exception;
    error = 1;
  }
  return valueResult(failed, result, error);
}

export function __rustier_js_own_keys(handle, failed) {
  let result;
  let error = 0;
  try {
    result = Reflect.ownKeys(values[handle >>> 0]);
  } catch (exception) {
    result = exception;
    error = 1;
  }
  return valueResult(failed, result, error);
}

export function __rustier_js_array_get(handle, index) {
  return __rustier_js_insert(values[handle >>> 0][memoryIndex(index)]);
}

export function __rustier_js_apply(
  handle,
  receiver,
  hasReceiver,
  args,
  len,
  failed,
) {
  let result;
  let error = 0;
  try {
    const argumentsList = argumentsFromMemory(args, len);
    const thisValue = hasReceiver ? values[receiver >>> 0] : undefined;
    result = Reflect.apply(values[handle >>> 0], thisValue, argumentsList);
  } catch (exception) {
    result = exception;
    error = 1;
  }
  return valueResult(failed, result, error);
}

export function __rustier_js_call(handle, method, args, len, failed) {
  let result;
  let error = 0;
  try {
    const object = values[handle >>> 0];
    const key = values[method >>> 0];
    const argumentsList = argumentsFromMemory(args, len);
    const functionValue = Reflect.get(object, key);
    result = Reflect.apply(functionValue, object, argumentsList);
  } catch (exception) {
    result = exception;
    error = 1;
  }
  return valueResult(failed, result, error);
}

export function __rustier_js_construct(handle, args, len, failed) {
  let result;
  let error = 0;
  try {
    const argumentsList = argumentsFromMemory(args, len);
    result = Reflect.construct(values[handle >>> 0], argumentsList);
  } catch (exception) {
    result = exception;
    error = 1;
  }
  return valueResult(failed, result, error);
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
