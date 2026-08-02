use crate::codegen::ir::{span::Span, type_::Type};

/// A field declared by a struct or enum variant.
#[salsa::tracked(debug)]
pub struct ShapeField<'db> {
    #[returns(ref)]
    pub name: String,
    #[tracked]
    pub type_: Type<'db>,
    #[tracked]
    pub is_readonly: bool,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
}

/// The field layout shared by structs and enum variants.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum Shape<'db> {
    Unit,
    /// Tuple fields, where private or hidden fields are represented by `None`.
    Tuple(Vec<Option<ShapeField<'db>>>),
    Plain {
        fields: Vec<ShapeField<'db>>,
        has_stripped_fields: bool,
    },
}

impl<'db> Shape<'db> {
    /// Iterates over the public lowered fields in their declaration order.
    pub fn fields(&self) -> impl Iterator<Item = ShapeField<'db>> + Clone + '_ {
        let (tuple_fields, plain_fields): (&[Option<ShapeField<'db>>], &[ShapeField<'db>]) =
            match self {
                Self::Unit => (&[], &[]),
                Self::Tuple(fields) => (fields, &[]),
                Self::Plain { fields, .. } => (&[], fields),
            };

        tuple_fields.iter().flatten().chain(plain_fields).copied()
    }
}
