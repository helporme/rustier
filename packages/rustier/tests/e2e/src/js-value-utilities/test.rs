use rustier_ext::runtime::js::{JsEnv, JsValue, Key};

pub fn strict_eq(left: JsValue, right: JsValue) -> bool {
    left.strict_eq(&right)
}

pub fn soft_eq(left: JsValue, right: JsValue) -> bool {
    left.soft_eq(&right)
}

pub fn read_string(value: JsValue) -> JsValue {
    let env = value.env();

    value
        .to_string()
        .map_or_else(|| env.null(), |text| env.new_string(text))
}

pub fn make_symbol(env: JsEnv, description: JsValue) -> JsValue {
    env.new_symbol(description.to_string().as_deref())
}

pub fn get(object: JsValue, key: JsValue) -> Result<JsValue, JsValue> {
    object.get(&key)
}

pub fn set(object: JsValue, key: JsValue, value: JsValue) -> Result<bool, JsValue> {
    object.set(key, &value)
}

pub fn get_number(object: JsValue, key: f64) -> Result<JsValue, JsValue> {
    object.get(key)
}

pub fn set_number(object: JsValue, key: f64, value: JsValue) -> Result<bool, JsValue> {
    object.set(key, &value)
}

pub fn get_index(object: JsValue, key: u32) -> Result<JsValue, JsValue> {
    object.get(key)
}

pub fn has(object: JsValue, key: JsValue) -> Result<bool, JsValue> {
    object.has(key)
}

pub fn delete(object: JsValue, key: JsValue) -> Result<bool, JsValue> {
    object.delete(key)
}

pub fn descriptor(object: JsValue, key: JsValue) -> Result<JsValue, JsValue> {
    object.get_own_property_descriptor(key)
}

pub fn define(object: JsValue, key: JsValue, descriptor: JsValue) -> Result<bool, JsValue> {
    object.define_property(key, &descriptor)
}

pub fn own_keys(object: JsValue) -> Result<JsValue, JsValue> {
    let env = object.env();
    let keys = object.own_keys()?;
    let result = env.new_object();

    result.set("length", &env.new_number(keys.len() as f64))?;

    for (index, key) in keys.into_iter().enumerate() {
        let value = match key {
            Key::Value(value) => value,
            Key::String(value) => env.new_string(value),
            Key::Number(value) => env.new_number(value),
        };

        result.set(index, &value)?;
    }

    Ok(result)
}

pub fn copy_properties(object: JsValue) -> Result<JsValue, JsValue> {
    let copy = object.env().new_object();

    for key in object.own_keys()? {
        let descriptor = object.get_own_property_descriptor(key.clone())?;

        copy.define_property(key, &descriptor)?;
    }

    Ok(copy)
}

pub fn call(object: JsValue, key: JsValue, arg: JsValue) -> Result<JsValue, JsValue> {
    object.call(key, &[arg])
}

pub fn construct(constructor: JsValue, args: JsValue) -> Result<JsValue, JsValue> {
    let count = args.get("length")?.as_f64().unwrap() as u32;
    let args = (0..count)
        .map(|index| args.get(index))
        .collect::<Result<Vec<_>, _>>()?;

    constructor.construct(&args)
}
