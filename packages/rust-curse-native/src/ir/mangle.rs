//! Shared mangling for the JS-side flat namespace exposed by napi-rs /
//! wasm-bindgen. Every printer that emits or references those names goes
//! through these helpers so the names stay consistent across targets.

use super::ItemPath;

/// `__cursed_<seg1>__<seg2>__..._<name>`
pub fn mangle_fn(path: &ItemPath) -> String {
    format!("__cursed_{}", path.segments.join("__"))
}

/// `__cursed_<seg1>__<seg2>__..._<name>__Ref`
pub fn mangle_ref(path: &ItemPath) -> String {
    format!("__cursed_{}__Ref", path.segments.join("__"))
}

/// `__cursed_<seg1>__<seg2>__..._<name>__Copy`
pub fn mangle_copy(path: &ItemPath) -> String {
    format!("__cursed_{}__Copy", path.segments.join("__"))
}

/// True when `item_path` lives directly inside `mod_segments` (i.e. the
/// module is the canonical owner of the item, not a re-exporter).
pub fn is_canonical_origin(item_path: &ItemPath, mod_segments: &[String]) -> bool {
    if item_path.segments.len() < 2 {
        return false;
    }
    &item_path.segments[..item_path.segments.len() - 1] == mod_segments
}
