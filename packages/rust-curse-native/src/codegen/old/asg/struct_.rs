use crate::codegen::asg::{ItemPath, Span, Type};

/// A struct field.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct StructField<'db> {
    pub name: String,
    pub ty: Type<'db>,
    pub span: Option<Span>,
    pub docs: Option<String>,
}

/// Shape of a struct or enum variant payload.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum StructShape<'db> {
    // other shapes are unsupported
    Plain {
        fields: Vec<StructField<'db>>,
        /// Whether the struct has also private fields.
        // todo: check the #[docs(hidden)] influence on private fields
        has_stripped_fields: bool,
    },
}

#[salsa::tracked(debug)]
pub struct StructDef<'db> {
    pub path: ItemPath<'db>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
    #[tracked]
    #[returns(ref)]
    pub shape: StructShape<'db>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}
