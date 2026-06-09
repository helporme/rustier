//! Abstract semantic graph (ASG) for rust.
//!
//! In essence it's a subset of rustdoc types with salsa annotations and all
//! things that rust-curse doesn't support (like generic fns) stripped away.
//! Lowering from the rustdoc is implemented in
//! [`crate::codegen::rustdoc::lower`].
//!
//! Besides, it's useful to loosen dependnecy on rustdoc json, since it's a
//! nightly feature and god knows what might change.

mod enum_;
mod fn_;
mod impl_;
mod item;
mod mod_;
mod struct_;
mod trait_;
mod type_;

pub use enum_::*;
pub use fn_::*;
pub use impl_::*;
pub use item::*;
pub use mod_::*;
pub use struct_::*;
pub use trait_::*;
pub use type_::*;

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct Span {
    /// `filename` interpretation depends on the originating crate: relative to
    /// the project root for the local crate, absolute under `~/.cargo/...` for
    /// registry deps, and so on. `None` if there is no source span available.
    pub filename: PathBuf,
    /// 1-indexed `(line, column)` begin/end of the span.
    pub begin: (usize, usize),
    /// 1-indexed `(line, column)` begin/end of the span.
    pub end: (usize, usize),
}