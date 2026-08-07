use miette::Diagnostic;
use thiserror::Error;

use super::{RustdocId, RustdocType, enums::EnumLowerError, structs::StructLowerError};
use crate::compiler::{
    db::Db,
    ir::rust::{Enum, Mod, Struct, Type, ValueType},
};

use rustdoc_types as doc;

impl<'db> Type<'db> {
    pub(super) fn lower(
        db: &'db dyn Db,
        type_: RustdocType<'db>,
        parent: Mod<'db>,
    ) -> Result<Self, Vec<TypeLowerError>> {
        let crate_ = type_.crate_(db);
        let crate_name = type_.crate_name(db);
        let type_ = type_.inner(db);

        match type_ {
            doc::Type::Primitive(name) => match lower_primitive(name) {
                Some(type_) => Ok(Type::Value(type_)),
                None => Err(vec![TypeLowerError::UnsupportedPrimitive {
                    name: name.clone(),
                }]),
            },
            doc::Type::ResolvedPath(path) => {
                let Some(summary) = crate_.paths.get(&path.id) else {
                    return Err(vec![TypeLowerError::UnsupportedType {
                        type_: type_.clone(),
                    }]);
                };

                if summary.path == ["alloc", "string", "String"] {
                    Ok(Type::Value(ValueType::String))
                } else if summary.kind == doc::ItemKind::Struct
                    && crate_.index.contains_key(&path.id)
                {
                    let id = RustdocId::new(db, crate_name, path.id);
                    let struct_ = Struct::lower(db, id, parent);
                    match struct_ {
                        Ok(s) => Ok(Type::Value(ValueType::Struct(s))),
                        Err(e) => Err(vec![TypeLowerError::StructLowerError(*e)]),
                    }
                } else if summary.kind == doc::ItemKind::Enum && crate_.index.contains_key(&path.id)
                {
                    let id = RustdocId::new(db, crate_name, path.id);
                    let enum_ = Enum::lower(db, id, parent);
                    match enum_ {
                        Ok(e) => Ok(Type::Value(ValueType::Enum(e))),
                        Err(e) => Err(vec![TypeLowerError::EnumLowerError(*e)]),
                    }
                } else {
                    Err(vec![TypeLowerError::UnsupportedType {
                        type_: type_.clone(),
                    }])
                }
            }
            doc::Type::BorrowedRef {
                lifetime,
                is_mutable,
                type_,
            } => {
                let mut diags = Vec::new();

                if lifetime.is_some() {
                    diags.push(TypeLowerError::UnsupportedLifetimeRef);
                }

                let type_ = type_.as_ref();
                let inner = match Self::lower(db, RustdocType::new(db, crate_name, type_), parent) {
                    Ok(Type::Value(inner)) => Some(inner),
                    Ok(Type::Borrowed { .. }) => {
                        diags.push(TypeLowerError::UnsupportedDoubleReference);
                        None
                    }
                    Err(inner_diags) => {
                        diags.extend(inner_diags);
                        None
                    }
                };

                match inner {
                    Some(inner) if diags.is_empty() => Ok(Type::Borrowed {
                        is_mutable: *is_mutable,
                        inner,
                    }),
                    _ => Err(diags),
                }
            }
            _ => Err(vec![TypeLowerError::UnsupportedType {
                type_: type_.clone(),
            }]),
        }
    }
}

fn lower_primitive<'db>(name: &str) -> Option<ValueType<'db>> {
    let type_ = match name {
        "bool" => ValueType::bool,
        "char" => ValueType::char,
        "str" => ValueType::Str,
        "i8" => ValueType::i8,
        "i16" => ValueType::i16,
        "i32" => ValueType::i32,
        "u8" => ValueType::u8,
        "u16" => ValueType::u16,
        "u32" => ValueType::u32,
        "f32" => ValueType::f32,
        "f64" => ValueType::f64,
        _ => return None,
    };

    Some(type_)
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum TypeLowerError {
    #[error("unsupported type: {type_:#?}")]
    #[diagnostic(code(rust_curse::unsupported_type), severity(Warning))]
    UnsupportedType { type_: doc::Type },

    #[error("the primitive type `{name}` is not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_primitive),
        help(
            "supported primitive types are bool, char, str, i8, i16, i32, u8, u16, u32, f32 and f64"
        ),
        severity(Warning)
    )]
    UnsupportedPrimitive { name: String },

    #[error("references with explicit lifetimes are not supported")]
    #[diagnostic(code(rust_curse::unsupported_lifetime_ref), severity(Warning))]
    UnsupportedLifetimeRef,

    #[error("double references are not supported")]
    #[diagnostic(
        code(rust_curse::unsupported_double_reference),
        help("use a single reference instead of a double reference"),
        severity(Warning)
    )]
    UnsupportedDoubleReference,

    #[error(transparent)]
    #[diagnostic(transparent)]
    StructLowerError(#[from] StructLowerError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    EnumLowerError(#[from] EnumLowerError),
}
