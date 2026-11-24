use rustier::{FromJs, JsCallError, JsValue, ToJs, js};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(js::FromJs, js::ToJs)]
#[js(case = "camelCase")]
pub struct Record<T> {
    some_value: T,
    #[js(rename = "__proto__")]
    magic_key: String,
    optional: Option<bool>,
    #[js(skip)]
    _ignored: NotConvertible,
}

#[derive(Default)]
struct NotConvertible;

#[derive(FromJs, ToJs)]
#[js(case = "snake_case")]
pub enum Fruit {
    GreenApple,
    #[js(rename = "pear!")]
    Pear,
}

#[derive(FromJs, ToJs)]
#[js(tag = "kind")]
pub enum Item {
    Empty,
    Pair(Fruit, i16),
    Named { count: u32 },
}

#[derive(FromJs, ToJs)]
pub enum Batch<T> {
    Empty,
    One(T),
    Many { items: Vec<T> },
}

#[derive(FromJs, ToJs)]
pub struct Ticket<T> {
    #[js(handle, rename = "token")]
    pub payload: T,
    pub count: u8,
}

#[derive(FromJs, ToJs)]
pub enum Action<T> {
    Store(#[js(handle)] T),
}

#[derive(FromJs, ToJs)]
pub struct Tree<T> {
    value: T,
    children: Vec<Self>,
}

#[derive(FromJs, ToJs)]
pub struct Collections {
    array: [i32; 2],
    tuple: (f32, bool),
    vec: Vec<char>,
    set: HashSet<u8>,
    ordered_set: BTreeSet<String>,
    record: HashMap<String, Item>,
    map: BTreeMap<i32, u8>,
}

pub fn round_trip<T: FromJs + ToJs>(input: JsValue) -> Result<JsValue, JsCallError> {
    let env = input.env();
    T::from_js(input)?.to_js(env)
}
