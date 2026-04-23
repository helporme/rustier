//! IR used to print the target representation (ts/wasm/napi).
//!
//! Main difference from the rustdoc representation is that rust curse IR
//! distinguish between **"ref" struct** and **"copy" struct** in the same vein
//! as C# distinguish between class and struct.

// Note: All supported types are stated explicitly so IR-printing is error free, also it's nice to
// read.

#![allow(non_camel_case_types)]

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Copy, Clone, Default, Debug, Hash, PartialEq, Eq)]
        pub struct $name(pub u32);
    };
}

id_type!(TypeId);
id_type!(StructId);
id_type!(FnId);
id_type!(ModId);

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

/// Structural type term. Nominal defs are referenced by id.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Unit,
    Boolean,
    Char,
    Number(NumberType),
    String,
    Str,
    Option(TypeId),
    Result(TypeId, TypeId),
    Array(TypeId, usize),
    Slice(TypeId),
    Tuple(Vec<TypeId>),
    Vec(TypeId),
    HashSet(TypeId),
    BTreeSet(TypeId),
    HashMap(TypeId, TypeId),
    BTreeMap(TypeId, TypeId),
    Struct(StructId),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NumberType {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnDef {
    pub path: ItemPath,
    pub params: Vec<FnParam>,
    pub return_type: TypeId,
    pub docs: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnParam {
    pub name: String,
    pub mode: ParamMode,
    pub type_id: TypeId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParamMode {
    ByValue,
    ByRef,
    ByMut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructDef {
    pub path: ItemPath,
    pub boundary: StructBoundary,
    pub shape: StructShape,
    pub docs: Option<String>,
}

/// How the struct crosses the js/rust boundary: as an opaque handle
/// (Ref) or by full serialization (Copy).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StructBoundary {
    Ref { ctor: Ctor },
    Copy,
}

/// Shape of a struct or enum variant payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StructShape {
    Unit,
    Tuple(Vec<TypeId>),
    Struct(Vec<StructField>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ctor {
    /// `new` function defined in an `impl` block is used to construct ref
    /// struct.
    Explicit(FnId),
    /// struct expression (e.g. `Foo { a, b, c }`) is used to construct the ref
    /// struct.
    Implicit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructField {
    pub name: String,
    pub type_id: TypeId,
    pub docs: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModDef {
    pub path: ItemPath,
    pub exports: Vec<Export>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Export {
    pub name: String,
    pub kind: ExportKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportKind {
    Fn(FnId),
    Struct(StructId),
    Mod(ModId),
}

#[derive(Clone, Debug, Default)]
pub struct IR {
    pub root: ModId,
    pub types: Vec<Type>,
    pub structs: Vec<StructDef>,
    pub fns: Vec<FnDef>,
    pub mods: Vec<ModDef>,
}

impl IR {
    pub fn type_(&self, id: TypeId) -> &Type {
        &self.types[id.0 as usize]
    }
    pub fn struct_(&self, id: StructId) -> &StructDef {
        &self.structs[id.0 as usize]
    }
    pub fn fn_(&self, id: FnId) -> &FnDef {
        &self.fns[id.0 as usize]
    }
    pub fn mod_(&self, id: ModId) -> &ModDef {
        &self.mods[id.0 as usize]
    }
}

pub mod from_rustdoc;
pub mod ts;
