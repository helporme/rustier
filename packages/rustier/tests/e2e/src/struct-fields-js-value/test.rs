use rustier_rt::js::JsValue;

#[derive(Clone)]
pub struct JsMessage {
    pub payload: JsValue,
}

impl JsMessage {
    pub fn new(payload: JsValue) -> Self {
        Self { payload }
    }

    pub fn replace(&mut self, payload: JsValue) -> JsValue {
        std::mem::replace(&mut self.payload, payload)
    }
}
