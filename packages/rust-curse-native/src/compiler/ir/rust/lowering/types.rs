use miette::Diagnostic;
use ra_ap_hir::{Adt, GenericDef, HirDisplay as _, Mutability, Trait, Type as HirType};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    ir::rust::{Enum, Struct, Type, ValueType},
};

pub(super) fn lower_type<'db>(
    db: &'db dyn Db,
    type_: &HirType<'db>,
    crate_: ra_ap_hir::Crate,
) -> Result<Type<'db>, TypeLowerErrorUnsupported> {
    if let Some((inner, mutability)) = type_.as_reference() {
        let Type::Value(inner) = lower_type(db, &inner, crate_)? else {
            return Err(TypeLowerErrorUnsupported::DoubleReference);
        };
        return Ok(Type::Borrowed {
            is_mutable: mutability == Mutability::Mut,
            inner,
        });
    }

    if let Some(builtin) = type_.as_builtin() {
        let name = builtin.name().display(db, crate_.edition(db)).to_string();
        let value = match name.as_str() {
            "bool" => ValueType::bool,
            "char" => ValueType::char,
            "str" => ValueType::Str,
            "i8" => ValueType::i8,
            "u8" => ValueType::u8,
            "i16" => ValueType::i16,
            "u16" => ValueType::u16,
            "i32" => ValueType::i32,
            "u32" => ValueType::u32,
            "f32" => ValueType::f32,
            "f64" => ValueType::f64,
            _ => return Err(TypeLowerErrorUnsupported::Primitive { name }),
        };
        return Ok(Type::Value(value));
    }

    let Some(adt) = type_.as_adt() else {
        return Err(unsupported_type(db, type_, crate_));
    };
    if !GenericDef::Adt(adt).params(db).is_empty() {
        return Err(unsupported_type(db, type_, crate_));
    }
    let value = match adt {
        Adt::Struct(struct_)
            if struct_.name(db).display(db, crate_.edition(db)).to_string() == "String"
                && struct_
                    .module(db)
                    .path_segments(db)
                    .map(|name| name.display(db, crate_.edition(db)).to_string())
                    .collect::<Vec<_>>()
                    .ends_with(&["string".to_owned()]) =>
        {
            ValueType::String
        }
        Adt::Struct(struct_) => ValueType::Struct(Struct::new(db, struct_)),
        Adt::Enum(enum_) => ValueType::Enum(Enum::new(db, enum_)),
        Adt::Union(_) => return Err(unsupported_type(db, type_, crate_)),
    };
    Ok(Type::Value(value))
}

pub(super) fn is_clonable(db: &dyn Db, type_: &HirType<'_>, crate_: ra_ap_hir::Crate) -> bool {
    Trait::lang(db, crate_, ra_ap_hir::LangItem::Clone)
        .is_some_and(|clone| type_.impls_trait(db, clone, &[]))
}

fn unsupported_type(
    db: &dyn Db,
    type_: &HirType<'_>,
    crate_: ra_ap_hir::Crate,
) -> TypeLowerErrorUnsupported {
    TypeLowerErrorUnsupported::Type {
        type_: type_.display(db, crate_.to_display_target(db)).to_string(),
    }
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub(super) enum TypeLowerErrorUnsupported {
    #[error("unsupported type: {type_}")]
    #[diagnostic(code(rust_curse::unsupported_type), severity(Warning))]
    Type { type_: String },

    #[error("the primitive type `{name}` is not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_primitive),
        help(
            "supported primitive types are bool, char, str, i8, i16, i32, u8, u16, u32, f32 and f64"
        ),
        severity(Warning)
    )]
    Primitive { name: String },

    #[error("references with explicit lifetimes are not supported")]
    #[diagnostic(code(rust_curse::unsupported_lifetime_ref), severity(Warning))]
    LifetimeRef,

    #[error("double references are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_double_reference),
        help("use a single reference instead of a double reference"),
        severity(Warning)
    )]
    DoubleReference,
}
