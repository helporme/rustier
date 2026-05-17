//! Per-target IR used to print the target representation (ts/wasm/napi).
//!
//! Lowered from the [`super::asg::ASG`] via [`from_asg::IR::from_asg`].
//! Whereas the ASG mirrors rustdoc, the IR is shaped for codegen: it
//! will diverge from the ASG over time (e.g. flattened class shapes,
//! synthesised ctors/getters/setters) so each printer can stay simple.

#![allow(non_camel_case_types)]

pub mod from_asg;

use std::path::PathBuf;

/// A unique identifier for an item in the IR.
#[derive(Copy, Clone, Default, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Id(pub u32);

/// Complete path to an item as it appears in the source crate,
/// e.g. `crate::module::Type` is stored as `["crate", "module", "Type"]`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct ItemPath {
    pub segments: Vec<String>,
}

impl ItemPath {
    pub fn new(segments: Vec<String>) -> Self {
        Self { segments }
    }

    pub fn name(&self) -> Option<&str> {
        self.segments.last().map(String::as_str)
    }
}

/// Renders `path` as a Rust source path string. If the first segment
/// matches `local_crate`, it's rewritten to `crate` so the emitted code
/// is valid from inside that crate; otherwise segments are joined as-is,
/// preserving extern-crate references.
pub fn rust_path(path: &ItemPath, local_crate: &str) -> String {
    match path.segments.split_first() {
        Some((head, tail)) if head == local_crate => {
            if tail.is_empty() {
                "crate".to_string()
            } else {
                format!("crate::{}", tail.join("::"))
            }
        }
        _ => path.segments.join("::"),
    }
}

/// A structural type term. Nominal defs are referenced by id.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Primitive(Primitive),
    Struct(Id),
}

/// A primitive type (everything that isn't a nominal struct).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Primitive {
    Unit,
    Boolean,
    Char,
    String,
    Str,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    i64,
    u64,
    isize,
    usize,
    f32,
    f64,
}

/// A function definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnDef {
    pub path: ItemPath,
    pub span: Option<Span>,
    pub params: Vec<FnParam>,
    pub return_type: Type,
    pub docs: Option<String>,
}

/// A function parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnParam {
    pub name: String,
    pub mode: ParamMode,
    pub ty: Type,
}

/// A parameter mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParamMode {
    ByValue,
    ByRef,
    ByMut,
}

/// A struct definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructDef {
    pub path: ItemPath,
    pub span: Option<Span>,
    pub ctor: Ctor,
    /// `true` if the source crate exposes a `Clone` impl for this struct
    pub clonable: bool,
    pub shape: StructShape,
    pub docs: Option<String>,
}

/// Shape of a struct or enum variant payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StructShape {
    Unit,
    Tuple(Vec<Type>),
    Struct(Vec<StructField>),
}

/// A "constructor" for a ref struct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ctor {
    /// `new` function defined in an `impl` block is used to construct ref
    /// struct.
    Explicit(Id),
    /// struct expression (e.g. `Foo { a, b, c }`) is used to construct the ref
    /// struct.
    Implicit,
}

/// A struct field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructField {
    pub name: String,
    pub ty: Type,
    pub docs: Option<String>,
}

/// A module definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModDef {
    pub path: ItemPath,
    /// Source span where this module is declared.
    pub span: Option<Span>,
    pub exports: Vec<Export>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

/// An "export" from a module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Export {
    pub name: String,
    pub target: Id,
}

/// An item in the IR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Struct(StructDef),
    Fn(FnDef),
    Mod(ModDef),
}

/// The rust curse IR for a Rust crate.
#[derive(Clone, Debug, Default)]
pub struct IR {
    pub root: Id,
    pub crate_name: String,
    pub items: Vec<Item>,
}

impl IR {
    /// Gets an item by its ID, panics if the item is not found.
    pub fn get_unchecked(&self, id: Id) -> &Item {
        &self.items[id.0 as usize]
    }

    /// Gets a struct by its ID, panics if the item is not a struct.
    pub fn get_struct_unchecked(&self, id: Id) -> &StructDef {
        match self.get_unchecked(id) {
            Item::Struct(s) => s,
            other => panic!("expected struct at {id:?}, got {other:?}"),
        }
    }

    /// Gets a function by its ID, panics if the item is not a function.
    pub fn get_fn_unchecked(&self, id: Id) -> &FnDef {
        match self.get_unchecked(id) {
            Item::Fn(f) => f,
            other => panic!("expected fn at {id:?}, got {other:?}"),
        }
    }

    /// Gets a module by its ID, panics if the item is not a module.
    pub fn get_mod_unchecked(&self, id: Id) -> &ModDef {
        match self.get_unchecked(id) {
            Item::Mod(m) => m,
            other => panic!("expected mod at {id:?}, got {other:?}"),
        }
    }
}
