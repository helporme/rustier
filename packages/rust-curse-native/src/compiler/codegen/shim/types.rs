use crate::{
    compiler::{
        codegen::{
            api::Api,
            cast::{CastContinuation, IndirectCastRule},
            fragment::{FragmentOnce, Printer, fragment},
        },
        db::Db,
        ir::{
            rust::{AsItem, FieldName, Type, ValueType},
            shim::{ShimType, ShimTypeKind, ShimValueType, ShimValueTypeKind},
        },
    },
    frag,
};

impl<'db> ShimValueType<'db> {
    pub(super) fn render_type_decl(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self.kind {
            ShimValueTypeKind::bool => "bool".apply(p),
            ShimValueTypeKind::i8 => "i8".apply(p),
            ShimValueTypeKind::u8 => "u8".apply(p),
            ShimValueTypeKind::i16 => "i16".apply(p),
            ShimValueTypeKind::u16 => "u16".apply(p),
            ShimValueTypeKind::i32 => "i32".apply(p),
            ShimValueTypeKind::u32 => "u32".apply(p),
            ShimValueTypeKind::f64 => "f64".apply(p),
            ShimValueTypeKind::String => "String".apply(p),
            ShimValueTypeKind::JsAny => Api::get(db).js_type(db).apply(p),
            ShimValueTypeKind::Class(c) => c.symbol(db).apply(p),
        })
    }

    pub(super) fn uses_tethering(&self) -> bool {
        match self.kind {
            ShimValueTypeKind::Class(_) => true,
            ShimValueTypeKind::bool
            | ShimValueTypeKind::i8
            | ShimValueTypeKind::u8
            | ShimValueTypeKind::i16
            | ShimValueTypeKind::u16
            | ShimValueTypeKind::i32
            | ShimValueTypeKind::u32
            | ShimValueTypeKind::f64
            | ShimValueTypeKind::String
            | ShimValueTypeKind::JsAny => false,
        }
    }
}

impl<'db> ShimType<'db> {
    pub(super) fn render_type_decl(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self.kind {
            ShimTypeKind::Value(ty) => ty.render_type_decl(db).apply(p),
            ShimTypeKind::Borrowed(ty) => frag!(&{ ty.render_type_decl(db) }).apply(p),
        })
    }
}

impl FragmentOnce for &FieldName {
    fn apply(self, p: &mut Printer) {
        match self {
            FieldName::Idx(idx) => idx.apply(p),
            FieldName::Name(name) => name.apply(p),
        }
    }
}

impl<'db> ValueType<'db> {
    pub(super) fn render_fqn(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            ValueType::bool => "bool".apply(p),
            ValueType::i8 => "i8".apply(p),
            ValueType::u8 => "u8".apply(p),
            ValueType::i16 => "i16".apply(p),
            ValueType::u16 => "u16".apply(p),
            ValueType::i32 => "i32".apply(p),
            ValueType::u32 => "u32".apply(p),
            ValueType::f64 => "f64".apply(p),
            ValueType::String => "String".apply(p),
            ValueType::char => "char".apply(p),
            ValueType::f32 => "f32".apply(p),
            ValueType::Str => "str".apply(p),
            ValueType::Struct(struct_) => struct_.fqn(db).apply(p),
            ValueType::Enum(enum_) => enum_.fqn(db).apply(p),
        })
    }

    pub(super) fn render_mangled_name(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            ValueType::bool => "__bool".apply(p),
            ValueType::i8 => "__i8".apply(p),
            ValueType::u8 => "__u8".apply(p),
            ValueType::i16 => "__i16".apply(p),
            ValueType::u16 => "__u16".apply(p),
            ValueType::i32 => "__i32".apply(p),
            ValueType::u32 => "__u32".apply(p),
            ValueType::f32 => "__f32".apply(p),
            ValueType::f64 => "__f64".apply(p),
            ValueType::Str => "__str".apply(p),
            ValueType::String => "__string".apply(p),
            ValueType::char => "__char".apply(p),
            ValueType::Struct(s) => s.path(db).mangled(db).apply(p),
            ValueType::Enum(e) => e.path(db).mangled(db).apply(p),
        })
    }

    pub(super) fn uses_tethering(self) -> bool {
        match self {
            ValueType::bool
            | ValueType::char
            | ValueType::i8
            | ValueType::u8
            | ValueType::i16
            | ValueType::u16
            | ValueType::i32
            | ValueType::u32
            | ValueType::f32
            | ValueType::f64
            | ValueType::Str
            | ValueType::String => false,
            ValueType::Struct(_) | ValueType::Enum(_) => true,
        }
    }

    pub(super) fn allows_generic_borrowing_cast(self) -> bool {
        match self {
            ValueType::bool
            | ValueType::char
            | ValueType::i8
            | ValueType::u8
            | ValueType::i16
            | ValueType::u16
            | ValueType::i32
            | ValueType::u32
            | ValueType::f32
            | ValueType::f64
            | ValueType::Str
            | ValueType::String => true,
            ValueType::Struct(_) | ValueType::Enum(_) => false,
        }
    }
}

/// A rule to cast an owned source type to a reference source type.
pub struct CastOwnedTypeToBorrowedTypeRule;

impl<'db> IndirectCastRule<'db, Type<'db>, Type<'db>> for CastOwnedTypeToBorrowedTypeRule {
    fn predecessor(self, _db: &'db dyn Db, to: Type<'db>) -> Option<Type<'db>> {
        match to {
            Type::Borrowed { inner, .. } if inner.allows_generic_borrowing_cast() => {
                Some(Type::Value(inner))
            }
            _ => None,
        }
    }

    fn render<'fragment, Expr, Continue>(
        self,
        _db: &'db dyn Db,
        _from: Type<'db>,
        to: Type<'db>,
        in_expr: Expr,
        continue_with: Continue,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
        Continue: CastContinuation<'db>,
    {
        let Type::Borrowed { is_mutable, .. } = to else {
            unreachable!()
        };

        let borrowing = if is_mutable {
            frag!(&mut __cast_owned_value)
        } else {
            frag!(&__cast_owned_value)
        };
        let continue_with = continue_with.compile(borrowing);

        frag!(
            let mut __cast_owned_value = {in_expr};
            {continue_with}
        )
    }
}
