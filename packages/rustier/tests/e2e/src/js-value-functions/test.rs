use rustier_ext::runtime::js::{JsEnv, JsValue};

pub fn apply0(function: JsValue) -> Result<JsValue, JsValue> {
    function.apply(&[])
}

pub fn apply2(function: JsValue, first: JsValue, second: JsValue) -> Result<JsValue, JsValue> {
    function.apply(&[first, second])
}

pub fn apply_as0(function: JsValue, receiver: JsValue) -> Result<JsValue, JsValue> {
    function.apply_as(&receiver, &[])
}

pub fn apply_as2(
    function: JsValue,
    receiver: JsValue,
    first: JsValue,
    second: JsValue,
) -> Result<JsValue, JsValue> {
    function.apply_as(&receiver, &[first, second])
}

pub fn apply_many(env: JsEnv, function: JsValue, count: u32) -> Result<JsValue, JsValue> {
    let args = (0..count)
        .map(|value| env.new_number(f64::from(value)))
        .collect::<Vec<_>>();

    function.apply(&args)
}

pub fn call0(object: JsValue, method: String) -> Result<JsValue, JsValue> {
    object.call(&method, &[])
}

pub fn call2(
    object: JsValue,
    method: String,
    first: JsValue,
    second: JsValue,
) -> Result<JsValue, JsValue> {
    object.call(&method, &[first, second])
}

pub fn apply_failed(function: JsValue) -> bool {
    function.apply(&[]).is_err()
}

pub fn caught_apply(function: JsValue) -> JsValue {
    match function.apply(&[]) {
        Ok(value) | Err(value) => value,
    }
}

pub fn call_failed(object: JsValue, method: String) -> bool {
    object.call(&method, &[]).is_err()
}

pub fn caught_call(object: JsValue, method: String) -> JsValue {
    match object.call(&method, &[]) {
        Ok(value) | Err(value) => value,
    }
}
