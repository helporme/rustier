use rustier_ext::runtime::js::{JsEnv, JsValue};

pub fn type_name(value: JsValue) -> String {
    format!("{:?}", value.type_of())
}

pub fn read_number(value: JsValue) -> Option<f64> {
    value.as_f64()
}

pub fn read_boolean(value: JsValue) -> Option<bool> {
    value.as_bool()
}

pub fn read_string(value: JsValue) -> Option<String> {
    value.to_string()
}

pub fn make_string(env: JsEnv, value: String) -> JsValue {
    env.new_string(&value)
}

pub fn make_boolean(env: JsEnv, value: bool) -> JsValue {
    env.new_boolean(value)
}

pub fn make_null(env: JsEnv) -> JsValue {
    env.null()
}

pub fn make_undefined(env: JsEnv) -> JsValue {
    env.undefined()
}

pub fn local_string(env: JsEnv) -> String {
    env.new_string("\u{feff}ready\0🦀").to_string().unwrap()
}

pub fn string_after_allocation(env: JsEnv, text: String) -> JsValue {
    let value = env.new_string(&text);
    let allocation = vec![0_u8; 4 * 1024 * 1024];

    std::hint::black_box(&allocation);

    let extracted = value.to_string().unwrap();
    let result = env.new_string(&extracted);

    drop(allocation);

    result
}
