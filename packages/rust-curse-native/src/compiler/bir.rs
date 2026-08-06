use crate::compiler::ir::{Enum, EnumVariant, Fn, Struct, Type, ValueType};

#[salsa::interned(debug)]
pub struct ExternSymbol<'db> {
    #[returns(ref)]
    pub value: String,
}

#[salsa::tracked(debug)]
pub struct ExternClass<'db> {
    pub symbol: ExternSymbol<'db>,
    #[tracked]
    #[returns(ref)]
    pub kind: ExternClassKind<'db>,
    #[tracked]
    #[returns(ref)]
    pub fns: Vec<ExternFn<'db>>,
    #[tracked]
    #[returns(ref)]
    pub fields: Vec<ExternClassField<'db>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ExternClassKind<'db> {
    /// A plain class.
    Plain,
    /// A tag-discriminated union class, where some fields are usable only for
    /// specific tag values.
    TagDiscriminatedUnion {
        /// Name of the tag field used for discriminator values.
        tag: ExternSymbol<'db>,
        /// Names of the tag values.
        names: Vec<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct ExternClassField<'db> {
    pub symbol: ExternSymbol<'db>,
    pub in_type: ExternType<'db>,
    pub out_type: ExternValueType<'db>,
    pub is_readonly: bool,
    pub kind: ExternClassFieldKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ExternClassFieldKind {
    Plain,
    TagDiscriminated { discriminator: u32 },
}

#[salsa::tracked(debug)]
pub struct ExternFn<'db> {
    pub symbol: ExternSymbol<'db>,
    #[tracked]
    #[returns(ref)]
    pub params: Vec<ExternFnParam<'db>>,
    #[tracked]
    pub return_type: Option<ExternValueType<'db>>,
}

#[salsa::tracked(debug)]
pub struct ExternFnParam<'db> {
    pub type_: ExternType<'db>,
    #[tracked]
    pub is_self: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum ExternValueType<'db> {
    bool,
    i8,
    u8,
    i16,
    u16,
    i32,
    u32,
    f64,
    String,
    /// Represents the Rust boundary type configured by [`Api::js_type`] for
    /// arbitrary JavaScript values.
    JsAny,
    Class(ExternSymbol<'db>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
#[allow(non_camel_case_types)]
pub enum ExternType<'db> {
    Value(ExternValueType<'db>),
    Borrowed(ExternValueType<'db>),
}

impl<'db> ExternType<'db> {
    pub fn deref(self) -> ExternValueType<'db> {
        match self {
            ExternType::Value(value) => value,
            ExternType::Borrowed(value) => value,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub struct Bindings<'db> {
    pub items: Vec<BindingItem<'db>>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub enum BindingItem<'db> {
    Fn(BindingFn<'db>),
    Class(BindingClass<'db>),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub struct BindingFn<'db> {
    pub external: ExternFn<'db>,
    pub source_return_type: Option<ValueType<'db>>,
    pub params: Vec<BindingFnParam<'db>>,
    pub body: BindingFnBody<'db>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub struct BindingFnParam<'db> {
    pub external: ExternFnParam<'db>,
    pub source_type: Type<'db>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub enum BindingFnBody<'db> {
    FnCall(Fn<'db>),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub struct BindingClass<'db> {
    pub external: ExternClass<'db>,
    pub source_type: ValueType<'db>,
    pub fields: Vec<BindingClassField<'db>>,
    pub fns: Vec<BindingFn<'db>>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub struct BindingClassField<'db> {
    pub external: ExternClassField<'db>,
    pub source_type: ValueType<'db>,
    pub access: FieldAccess<'db>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub enum FieldAccess<'db> {
    StructAccess {
        struct_: Struct<'db>,
        field_name: String,
    },
    EnumVariantAccess {
        enum_: Enum<'db>,
        variant: EnumVariant<'db>,
        field_name: String,
    },
}
