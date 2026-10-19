use rustier_rt::js::JsValue;

pub fn some(value: JsValue) -> Option<JsValue> {
    Some(value)
}

pub fn none() -> Option<JsValue> {
    None
}

pub fn roundtrip(value: Option<JsValue>) -> Option<JsValue> {
    value
}

pub fn unwrap_or(value: Option<JsValue>, fallback: JsValue) -> JsValue {
    value.unwrap_or(fallback)
}
