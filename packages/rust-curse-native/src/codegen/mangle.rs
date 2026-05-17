//! Module mangling utilities.

use super::asg::ItemPath;

/// `__cursed_<seg1>__<seg2>__..._<name>`
pub fn mangle(path: &ItemPath) -> String {
    format!("__cursed_{}", path.segments.join("__"))
}

/// True when `item_path` lives directly inside `mod_segments` (i.e. the
/// module is the canonical owner of the item, not a re-exporter).
pub fn is_canonical_origin(item_path: &ItemPath, mod_segments: &[String]) -> bool {
    if item_path.segments.len() < 2 {
        return false;
    }
    &item_path.segments[..item_path.segments.len() - 1] == mod_segments
}
