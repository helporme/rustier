//! IR used to print the target representation (ts/wasm/napi).

/// A unique identifier for an item in the IR.
#[derive(Copy, Clone, Default, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct IrId(pub usize);

/// A structural type term. Nominal defs are referenced by id.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Primitive(Primitive),
    Class(IrId),
}

/// A function definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnDef {
    pub id: IrId,
    pub orig: AsgId,
    pub path: ItemPath,
    pub js_name: String,
    pub mangled_name: String,
    pub params: Vec<FnParam>,
    pub return_type: Type,
    pub needs_closure_scope: bool,
    pub receiver: Option<Receiver>,
    pub docs: Option<String>,
    pub span: Option<Span>,
}

/// A function parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnParam {
    pub name: String,
    pub mode: ParamMode,
    pub ty: Type,
    pub needs_stash: bool,
}

/// A struct or data-enum definition lowered to a JS class wrapper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassDef {
    pub id: IrId,
    pub orig: AsgId,
    pub path: ItemPath,
    pub js_name: String,
    pub mangled_name: String,
    pub ctor: Option<CtorDef>,
    pub struct_expr: Option<StructExpr>,
    pub methods: Vec<IrId>,
    pub of_escaped: bool,
    pub clonable: bool,
    pub shape: ClassShape,
    pub docs: Option<String>,
    pub span: Option<Span>,
}

/// Whether a [`ClassDef`] is backed by a Rust struct or a data enum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClassShape {
    Struct {
        fields: Vec<ClassField>,
    },
    Enum {
        variants: Vec<EnumVariantDef>,
        discriminator: String,
    },
}

/// A lowered data-enum variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumVariantDef {
    pub rust_name: String,
    pub tag: String,
    pub is_unit: bool,
    pub fields: Vec<ClassField>,
    pub docs: Option<String>,
    pub span: Option<Span>,
}

/// A field of a JS class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassField {
    pub orig_name: String,
    pub js_name: String,
    pub ty: Type,
    pub has_getter: bool,
    pub has_setter: bool,
    pub clone_on_get: bool,
    pub docs: Option<String>,
    pub span: Option<Span>,
}

/// User-written constructor (`pub fn new(...) -> Self`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CtorDef {
    pub params: Vec<FnParam>,
    pub span: Option<Span>,
}

/// Struct-expression init descriptor for `.of` function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructExpr {
    pub fields: Vec<FnParam>,
}

/// A module definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModDef {
    pub id: IrId,
    pub orig: AsgId,
    pub path: ItemPath,
    pub span: Option<Span>,
    pub uses: Vec<Use>,
}

/// A name brought into a module's public surface .
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Use {
    pub name: String,
    pub target: IrId,
}

/// An item in the IR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Class(ClassDef),
    Fn(FnDef),
    Mod(ModDef),
}
