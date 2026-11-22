use rustier::{JsCallError, JsEnv, JsValue, js};

pub fn nested_rs(env: JsEnv) -> Result<JsValue, JsCallError> {
    js!(return rs`1u32`).at(env)
}
