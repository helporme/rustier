use rustier_ext::runtime::js::{JsEnv, JsValue};

use crate::fn_js_value::ValueStore;

pub fn get(object: JsValue, key: String) -> Result<JsValue, JsValue> {
    object.get(&key)
}

pub fn set(object: JsValue, key: String, value: JsValue) -> Result<bool, JsValue> {
    object.set(&key, &value)
}

pub fn get_failed(object: JsValue, key: String) -> bool {
    object.get(&key).is_err()
}

pub fn set_failed(object: JsValue, key: String, value: JsValue) -> bool {
    object.set(&key, &value).is_err()
}

pub fn caught_get(object: JsValue, key: String) -> JsValue {
    match object.get(&key) {
        Ok(value) => value,
        Err(exception) => exception,
    }
}

pub fn caught_set(object: JsValue, key: String, value: JsValue) -> JsValue {
    match object.set(&key, &value) {
        Ok(accepted) => object.env().new_boolean(accepted),
        Err(exception) => exception,
    }
}

pub fn retain_property(object: JsValue, key: String) -> Result<ValueStore, JsValue> {
    object.get(&key).map(ValueStore::new)
}

pub fn make_record(env: JsEnv, value: JsValue) -> Result<JsValue, JsValue> {
    let object = env.new_object();

    object.set("value", &value)?;

    Ok(object)
}

pub fn grow_memory() {
    #[cfg(target_arch = "wasm32")]
    core::arch::wasm32::memory_grow::<0>(1);
}
