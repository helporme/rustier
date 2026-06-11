#[path = "utils/rustdoc.rs"]
pub(crate) mod rustdoc;

#[path = "utils/diags.rs"]
pub(crate) mod diags;

#[path = "utils/snap.rs"]
pub(crate) mod snap;

#[path = "utils/macros.rs"]
mod macros;
pub(crate) use macros::*;

#[path = "utils/ir_debug.rs"]
pub(crate) mod ir_debug;
