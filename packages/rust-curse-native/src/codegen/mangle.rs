//! Module mangling utilities.

use super::ir::ItemPath;

/// `__cursed_<seg1>__<seg2>__..._<name>`
pub fn mangle(path: &ItemPath) -> String {
    format!("__cursed_{}", path.segments.join("__"))
}

/// The wrapper's projection-root field. Fixed, no user input.
pub const ROOT_FIELD: &str = "__cursed_root";

/// Cache slot for a struct-typed field: `__cursed_cache_<userfield>`.
pub fn cache_field(user_field: &str) -> String {
    format!("__cursed_cache__{user_field}")
}

/// Cache slot for a class-typed enum variant field, mangled by `(variant,
/// field)` so different per-variant child types never share storage:
/// `__cursed_cache_<Variant>__<field>`.
pub fn variant_cache_field(variant: &str, user_field: &str) -> String {
    format!("__cursed_cache_{variant}__{user_field}")
}

/// Generated fn arg (ctor / setter value): `__cursed_arg_<userfield>`.
pub fn arg_name(user_field: &str) -> String {
    format!("__cursed_arg__{user_field}")
}

/// Field getter fn: `__cursed_get_<userfield>`.
pub fn getter_fn(user_field: &str) -> String {
    format!("__cursed_get__{user_field}")
}

/// Field setter fn: `__cursed_set_<userfield>`.
pub fn setter_fn(user_field: &str) -> String {
    format!("__cursed_set__{user_field}")
}

/// Method dispatch fn: `__cursed_method_<methodname>`.
pub fn method_fn(method_name: &str) -> String {
    format!("__cursed_method__{method_name}")
}

/// True when `item_path` lives directly inside `mod_segments` (i.e. the
/// module is the canonical owner of the item, not a re-exporter).
pub fn is_canonical_origin(item_path: &ItemPath, mod_segments: &[String]) -> bool {
    if item_path.segments.len() < 2 {
        return false;
    }
    &item_path.segments[..item_path.segments.len() - 1] == mod_segments
}
