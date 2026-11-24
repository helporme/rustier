//! Conversions between Rust values and JavaScript data.

#[doc(hidden)]
pub use crate::js_call::write_handle;
use crate::{JsCallError, JsEnv, JsValue, js, js::JsValueType, transport::JsEnvHandle};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    hash::{BuildHasher, Hash},
};

type Result<T> = std::result::Result<T, JsCallError>;

/// Converts an owned Rust value into a JavaScript value.
///
/// Use `#[derive(ToJs)]` to automatically implement for structs and enums.
///
/// ```
/// # use rustier_runtime as rustier;
/// use rustier::{JsCallError, JsEnv, JsValue, ToJs};
///
/// // Produces the JavaScript array ["Fluffy", 1].
/// fn cat(env: JsEnv) -> Result<JsValue, JsCallError> {
///     ("Fluffy", 1u32).to_js(env)
/// }
/// ```
pub trait ToJs {
    fn to_js(self, env: JsEnv) -> Result<JsValue>;
}

/// Reads an owned Rust value from JavaScript.
///
/// Use `#[derive(FromJs)]` for structs and enums.
/// ```
/// # use rustier_runtime as rustier;
/// use rustier::{FromJs, JsCallError, JsValue};
///
/// // Accepts [3, 4], but not [3, 4.5] or an array of a different length.
/// fn coordinates(value: JsValue) -> Result<[i32; 2], JsCallError> {
///     <[i32; 2]>::from_js(value)
/// }
/// ```
pub trait FromJs: Sized {
    fn from_js(value: JsValue) -> Result<Self>;
}

impl ToJs for u8 {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        js!(return @{self}).at(env)
    }
}

impl FromJs for u8 {
    fn from_js(value: JsValue) -> Result<Self> {
        value
            .as_u16()
            .and_then(|n| n.try_into().ok())
            .ok_or_else(|| JsCallError::new("expected u8"))
    }
}

impl ToJs for i8 {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        Ok(env.new_number(self.into()))
    }
}

impl FromJs for i8 {
    fn from_js(value: JsValue) -> Result<Self> {
        value
            .as_i16()
            .and_then(|n| n.try_into().ok())
            .ok_or_else(|| JsCallError::new("expected i8"))
    }
}

impl ToJs for u16 {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        Ok(env.new_number(self.into()))
    }
}

impl FromJs for u16 {
    fn from_js(value: JsValue) -> Result<Self> {
        value
            .as_u16()
            .ok_or_else(|| JsCallError::new("expected u16"))
    }
}
impl ToJs for i16 {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        Ok(env.new_number(self.into()))
    }
}
impl FromJs for i16 {
    fn from_js(value: JsValue) -> Result<Self> {
        value
            .as_i16()
            .ok_or_else(|| JsCallError::new("expected i16"))
    }
}
impl ToJs for u32 {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        Ok(env.new_number(self.into()))
    }
}
impl FromJs for u32 {
    fn from_js(value: JsValue) -> Result<Self> {
        value
            .as_u32()
            .ok_or_else(|| JsCallError::new("expected u32"))
    }
}
impl ToJs for i32 {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        Ok(env.new_number(self.into()))
    }
}
impl FromJs for i32 {
    fn from_js(value: JsValue) -> Result<Self> {
        value
            .as_i32()
            .ok_or_else(|| JsCallError::new("expected i32"))
    }
}
impl ToJs for f32 {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        Ok(env.new_number(self.into()))
    }
}
impl FromJs for f32 {
    fn from_js(value: JsValue) -> Result<Self> {
        let n = value
            .as_f64()
            .ok_or_else(|| JsCallError::new("expected f32"))?;
        let rounded = n as f32;
        if n.is_finite() && !rounded.is_finite() {
            return Err(JsCallError::new("number outside f32 range"));
        }
        Ok(rounded)
    }
}
impl ToJs for f64 {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        Ok(env.new_number(self))
    }
}
impl FromJs for f64 {
    fn from_js(value: JsValue) -> Result<Self> {
        value
            .as_f64()
            .ok_or_else(|| JsCallError::new("expected f64"))
    }
}
macro_rules! bigint_conversions {
    ($($ty:ty => $new:ident($wire:ty), |$v:ident| $read:expr;)+) => {$(
        impl ToJs for $ty {
            fn to_js(self, env: JsEnv) -> Result<JsValue> {
                Ok(JsEnvHandle::from(env).$new(self as $wire).into())
            }
        }
        impl FromJs for $ty {
            fn from_js($v: JsValue) -> Result<Self> {
                ($read).ok_or_else(|| JsCallError::new(concat!("expected ", stringify!($ty))))
            }
        }
    )+};
}
bigint_conversions! {
    u64 => new_u64(u64), |v| v.as_u64();
    i64 => new_i64(i64), |v| v.as_i64();
    u128 => new_u128(u128), |v| v.as_u128();
    i128 => new_i128(i128), |v| v.as_i128();
    usize => new_u64(u64), |v| v.as_usize();
    isize => new_i64(i64), |v| v.as_i64().and_then(|n| isize::try_from(n).ok());
}
impl ToJs for bool {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        Ok(env.new_boolean(self))
    }
}
impl FromJs for bool {
    fn from_js(value: JsValue) -> Result<Self> {
        value
            .as_bool()
            .ok_or_else(|| JsCallError::new("expected bool"))
    }
}
impl ToJs for String {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        Ok(env.new_string(self))
    }
}
impl FromJs for String {
    fn from_js(value: JsValue) -> Result<Self> {
        value
            .to_string()
            .ok_or_else(|| JsCallError::new("expected String"))
    }
}
impl ToJs for char {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        Ok(env.new_string(self.encode_utf8(&mut [0; 4])))
    }
}
impl FromJs for char {
    fn from_js(value: JsValue) -> Result<Self> {
        value
            .to_string()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| JsCallError::new("expected char"))
    }
}
impl ToJs for () {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        Ok(env.undefined())
    }
}
impl FromJs for () {
    fn from_js(value: JsValue) -> Result<Self> {
        matches!(value.type_of(), JsValueType::Undefined | JsValueType::Null)
            .then_some(())
            .ok_or_else(|| JsCallError::new("expected null or undefined"))
    }
}

impl ToJs for &str {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        Ok(env.new_string(self))
    }
}

impl ToJs for JsValue {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        if self.env() != env {
            return Err(JsCallError::new("JsValue belongs to another environment"));
        }
        Ok(self)
    }
}

impl FromJs for JsValue {
    fn from_js(value: JsValue) -> Result<Self> {
        Ok(value)
    }
}

impl<T: ToJs> ToJs for Option<T> {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        match self {
            Some(value) => value.to_js(env),
            None => Ok(env.undefined()),
        }
    }
}

impl<T: FromJs> FromJs for Option<T> {
    fn from_js(value: JsValue) -> Result<Self> {
        match value.type_of() {
            JsValueType::Undefined | JsValueType::Null => Ok(None),
            _ => T::from_js(value).map(Some),
        }
    }
}

// Missing and explicitly undefined are distinct for required fields such as JsValue.
trait MissingField: Sized {
    fn missing(key: &str) -> Result<Self>;
}
impl<T> MissingField for T {
    default fn missing(key: &str) -> Result<Self> {
        Err(JsCallError::new(format!("missing required field `{key}`")))
    }
}
impl<T> MissingField for Option<T> {
    fn missing(_: &str) -> Result<Self> {
        Ok(None)
    }
}

#[doc(hidden)]
pub fn read_field<T: FromJs>(object: &JsValue, key: &str) -> Result<T> {
    match own_field(object, key)? {
        Some(value) => T::from_js(value),
        None => T::missing(key),
    }
}

#[doc(hidden)]
pub fn read_handle<T: 'static>(object: &JsValue, key: &str) -> Result<T> {
    let field = own_field(object, key)?
        .ok_or_else(|| JsCallError::new(format!("missing required field `{key}`")))?;
    let handle = field.as_rust_handle::<T>().ok_or_else(|| {
        JsCallError::new(format!(
            "expected Rust handle for `{}` in field `{key}`",
            std::any::type_name::<T>()
        ))
    })?;
    handle.take().ok_or_else(|| {
        JsCallError::new(format!("handle in field `{key}` was moved or is borrowed"))
    })
}

#[doc(hidden)]
pub fn check_object(value: &JsValue) -> Result<()> {
    if value.type_of() != JsValueType::Object {
        return Err(JsCallError::new("expected an object"));
    }
    Ok(())
}

#[doc(hidden)]
pub fn own_field(object: &JsValue, key: &str) -> Result<Option<JsValue>> {
    let descriptor =
        js!(return Object.getOwnPropertyDescriptor(@{object.clone()}, @{key.to_owned()}))
            .at(object.env())?;
    if descriptor.type_of() == JsValueType::Undefined {
        return Ok(None);
    }
    // Read the captured descriptor: a proxy may remove the property between
    // looking it up and reading it. Never fall back to an inherited property.
    let value = js!(
        const d = @{descriptor};
        return Object.hasOwn(d, "value") ? d.value
            : d.get === undefined ? undefined : Reflect.apply(d.get, @{object.clone()}, []);
    )
    .at(object.env())?;
    Ok(Some(value))
}

#[doc(hidden)]
pub fn put(object: &JsValue, key: &str, value: JsValue) -> Result<()> {
    js!(Object.defineProperty(@{object.clone()}, @{key.to_owned()}, {
        __proto__: null, value: @{value}, writable: true, enumerable: true, configurable: true
    }))
    .at(object.env())
}

#[doc(hidden)]
pub fn array(env: JsEnv) -> Result<JsValue> {
    js!(return []).at(env)
}

fn length(value: &JsValue) -> Result<usize> {
    let len = js!(return Array.isArray(@{value.clone()}) ? @{value.clone()}.length : null)
        .at(value.env())?;
    len.as_u32()
        .map(|n| n as usize)
        .ok_or_else(|| JsCallError::new("expected an array"))
}

#[doc(hidden)]
pub fn check_length(value: &JsValue, expected: usize) -> Result<()> {
    let len = length(value)?;
    if len != expected {
        return Err(JsCallError::new(format!(
            "expected array length {expected}, got {len}"
        )));
    }
    Ok(())
}

fn write_array<T: ToJs>(items: impl IntoIterator<Item = T>, env: JsEnv) -> Result<JsValue> {
    let out = array(env)?;
    for (index, item) in items.into_iter().enumerate() {
        put(&out, &index.to_string(), item.to_js(env)?)?;
    }
    Ok(out)
}

fn element(array: &JsValue, index: usize) -> Result<JsValue> {
    own_field(array, &index.to_string())?
        .ok_or_else(|| JsCallError::new(format!("missing array element {index}")))
}

impl<T: ToJs> ToJs for Vec<T> {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        write_array(self, env)
    }
}

impl<T: FromJs> FromJs for Vec<T> {
    fn from_js(value: JsValue) -> Result<Self> {
        (0..length(&value)?)
            .map(|i| T::from_js(element(&value, i)?))
            .collect()
    }
}

impl<T: ToJs, const N: usize> ToJs for [T; N] {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        write_array(self, env)
    }
}

impl<T: FromJs, const N: usize> FromJs for [T; N] {
    fn from_js(value: JsValue) -> Result<Self> {
        check_length(&value, N)?;
        let items: Vec<_> = (0..N)
            .map(|i| T::from_js(element(&value, i)?))
            .collect::<Result<_>>()?;
        Ok(items.try_into().unwrap_or_else(|_| unreachable!()))
    }
}

macro_rules! tuples {
    (@grow [$($seen:ident:$old:tt),*] $ty:ident:$idx:tt $(,$rest:ident:$next:tt)*) => {
        tuples!(@impl $($seen:$old,)* $ty:$idx);
        tuples!(@grow [$($seen:$old,)* $ty:$idx] $($rest:$next),*);
    };
    (@grow [$($seen:ident:$old:tt),*]) => {};
    (@impl $($ty:ident:$idx:tt),+) => {
        impl<$($ty: ToJs),+> ToJs for ($($ty,)+) {
            fn to_js(self, env: JsEnv) -> Result<JsValue> {
                let out = array(env)?;
                $(put(&out, stringify!($idx), self.$idx.to_js(env)?)?;)+
                Ok(out)
            }
        }

        impl<$($ty: FromJs),+> FromJs for ($($ty,)+) {
            fn from_js(value: JsValue) -> Result<Self> {
                check_length(&value, [$(stringify!($idx)),+].len())?;
                Ok(($($ty::from_js(element(&value, $idx)?)?,)+))
            }
        }
    };
}

tuples!(@grow [] A:0, B:1, C:2, D:3, E:4, F:5, G:6, H:7, I:8, J:9, K:10, L:11);

fn write_set<T: ToJs>(items: impl IntoIterator<Item = T>, env: JsEnv) -> Result<JsValue> {
    let items = write_array(items, env)?;
    js!(return new Set(@{items})).at(env)
}

fn read_set<T: FromJs, C: FromIterator<T>>(value: JsValue) -> Result<C> {
    let env = value.env();
    let items = js!(return Array.from(Set.prototype.values.call(@{value}))).at(env)?;
    (0..length(&items)?)
        .map(|i| T::from_js(element(&items, i)?))
        .collect()
}

impl<T: ToJs, S> ToJs for HashSet<T, S> {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        write_set(self, env)
    }
}

impl<T: FromJs + Eq + Hash, S: BuildHasher + Default> FromJs for HashSet<T, S> {
    fn from_js(value: JsValue) -> Result<Self> {
        read_set(value)
    }
}

impl<T: ToJs> ToJs for BTreeSet<T> {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        write_set(self, env)
    }
}

impl<T: FromJs + Ord> FromJs for BTreeSet<T> {
    fn from_js(value: JsValue) -> Result<Self> {
        read_set(value)
    }
}

fn write_record<K: AsRef<str>, V: ToJs>(
    entries: impl IntoIterator<Item = (K, V)>,
    env: JsEnv,
) -> Result<JsValue> {
    let object = env.new_object();
    for (key, value) in entries {
        put(&object, key.as_ref(), value.to_js(env)?)?;
    }
    Ok(object)
}

fn read_record<V: FromJs, C: FromIterator<(String, V)>>(value: JsValue) -> Result<C> {
    let env = value.env();
    let entries = js!(
        const obj = @{value};
        if (obj === null || typeof obj !== "object") throw new TypeError("expected a record");
        const proto = Object.getPrototypeOf(obj);
        if (proto !== null && Object.getPrototypeOf(proto) !== null) throw new TypeError("expected a record");
        return Object.entries(obj);
    ).at(env)?;
    read_entries(entries)
}

fn write_map<K: ToJs, V: ToJs>(
    items: impl IntoIterator<Item = (K, V)>,
    env: JsEnv,
) -> Result<JsValue> {
    let entries = write_array(items, env)?;
    js!(return new Map(@{entries})).at(env)
}

fn read_map<K: FromJs, V: FromJs, C: FromIterator<(K, V)>>(value: JsValue) -> Result<C> {
    let env = value.env();
    let entries = js!(return Array.from(Map.prototype.entries.call(@{value}))).at(env)?;
    read_entries(entries)
}

fn read_entries<K: FromJs, V: FromJs, C: FromIterator<(K, V)>>(entries: JsValue) -> Result<C> {
    (0..length(&entries)?)
        .map(|i| <(K, V)>::from_js(element(&entries, i)?))
        .collect()
}

impl<K: ToJs, V: ToJs, S> ToJs for HashMap<K, V, S> {
    default fn to_js(self, env: JsEnv) -> Result<JsValue> {
        write_map(self, env)
    }
}

impl<V: ToJs, S> ToJs for HashMap<String, V, S> {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        write_record(self, env)
    }
}

impl<V: ToJs, S> ToJs for HashMap<&str, V, S> {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        write_record(self, env)
    }
}

impl<K: FromJs + Eq + Hash, V: FromJs, S: BuildHasher + Default> FromJs for HashMap<K, V, S> {
    default fn from_js(value: JsValue) -> Result<Self> {
        read_map(value)
    }
}

impl<V: FromJs, S: BuildHasher + Default> FromJs for HashMap<String, V, S> {
    fn from_js(value: JsValue) -> Result<Self> {
        read_record(value)
    }
}

impl<K: ToJs, V: ToJs> ToJs for BTreeMap<K, V> {
    default fn to_js(self, env: JsEnv) -> Result<JsValue> {
        write_map(self, env)
    }
}

impl<V: ToJs> ToJs for BTreeMap<String, V> {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        write_record(self, env)
    }
}

impl<V: ToJs> ToJs for BTreeMap<&str, V> {
    fn to_js(self, env: JsEnv) -> Result<JsValue> {
        write_record(self, env)
    }
}

impl<K: FromJs + Ord, V: FromJs> FromJs for BTreeMap<K, V> {
    default fn from_js(value: JsValue) -> Result<Self> {
        read_map(value)
    }
}

impl<V: FromJs> FromJs for BTreeMap<String, V> {
    fn from_js(value: JsValue) -> Result<Self> {
        read_record(value)
    }
}
