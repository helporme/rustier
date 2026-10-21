use rustier_ext::runtime::js::JsEnv;
pub use rustier_ext::runtime::js::JsValue as Value;

pub type Alias = Value;

pub fn identity(value: Alias) -> Alias {
    value
}

pub fn choose(first: Value, _env: JsEnv, flag: bool, second: Alias) -> Value {
    if flag { first } else { second }
}

pub fn make_number(env: JsEnv, value: f64) -> Value {
    env.new_number(value)
}

pub fn make_object(env: JsEnv) -> Value {
    env.new_object()
}

pub fn reject(value: Value) -> Result<(), Value> {
    Err(value)
}

pub fn accept(value: Value) -> Result<Value, String> {
    Ok(value)
}

pub fn checked(value: Value, _index: u8) -> Value {
    value
}

pub struct ValueStore {
    value: Option<Value>,
}

impl ValueStore {
    pub fn new(value: Value) -> Self {
        Self { value: Some(value) }
    }

    pub fn get(&self) -> Value {
        self.value.as_ref().unwrap().clone()
    }

    pub fn take(&mut self) -> Value {
        self.value.take().unwrap()
    }

    pub fn clear(&mut self) {
        self.value = None;
    }
}
