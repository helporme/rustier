use rustier_ext::runtime::js::JsValue;

#[derive(Clone)]
pub struct OptionalMessage {
    pub payload: Option<JsValue>,
}

impl OptionalMessage {
    pub fn new(payload: Option<JsValue>) -> Self {
        Self { payload }
    }

    pub fn replace(&mut self, payload: Option<JsValue>) -> Option<JsValue> {
        std::mem::replace(&mut self.payload, payload)
    }
}
