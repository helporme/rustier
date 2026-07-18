use miette::Diagnostic;
use rustdoc_types::{ItemKind, Type};
use thiserror::Error;

use crate::codegen::{
    db::Db,
    ir::{
        IrId,
        type_::{Primitive, Type as IrType},
    },
    rustdoc::{CrateExt, RustdocType},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum LoweredType<'db> {
    Moved(IrType<'db>),
    Borrowed {
        type_: IrType<'db>,
        is_mutable: bool,
    },
}

/// Lowers the rustdoc type to the IR type.
#[salsa::tracked]
pub fn lower_type<'db>(
    db: &'db dyn Db,
    id: RustdocType<'db>,
) -> Result<LoweredType<'db>, Vec<TypeLowerError>> {
    let type_ = id.type_(db);
    let crate_ = id.crate_(db);

    match type_ {
        Type::Primitive(name) => match lower_primitive(name) {
            Some(type_) => Ok(LoweredType::Moved(type_)),
            None => Err(vec![TypeLowerError::UnsupportedPrimitive {
                name: name.clone(),
            }]),
        },
        Type::ResolvedPath(path) => {
            let Some(summary) = crate_.paths.get(&path.id) else {
                return Err(vec![TypeLowerError::UnsupportedType {
                    type_: type_.clone(),
                }]);
            };

            if summary.path == ["alloc", "string", "String"] {
                Ok(LoweredType::Moved(IrType::String))
            } else if summary.kind == ItemKind::Struct && crate_.index.contains_key(&path.id) {
                Ok(LoweredType::Moved(IrType::Struct(IrId::new(
                    db,
                    &summary.path,
                ))))
            } else if summary.kind == ItemKind::Enum && crate_.index.contains_key(&path.id) {
                Ok(LoweredType::Moved(IrType::Enum(IrId::new(
                    db,
                    &summary.path,
                ))))
            } else {
                Err(vec![TypeLowerError::UnsupportedType {
                    type_: type_.clone(),
                }])
            }
        }
        Type::BorrowedRef {
            lifetime,
            is_mutable,
            type_,
        } => {
            let mut diags = Vec::new();

            if lifetime.is_some() {
                diags.push(TypeLowerError::UnsupportedLifetimeRef);
            }

            let inner = match lower_type(db, crate_.ty(db, type_)) {
                Ok(LoweredType::Moved(inner)) => Some(inner),
                Ok(LoweredType::Borrowed { .. }) => {
                    diags.push(TypeLowerError::UnsupportedDoubleReference);
                    None
                }
                Err(inner_diags) => {
                    diags.extend(inner_diags);
                    None
                }
            };

            match inner {
                Some(inner) if diags.is_empty() => Ok(LoweredType::Borrowed {
                    type_: inner,
                    is_mutable: *is_mutable,
                }),
                _ => Err(diags),
            }
        }
        _ => Err(vec![TypeLowerError::UnsupportedType {
            type_: type_.clone(),
        }]),
    }
}

fn lower_primitive<'db>(name: &str) -> Option<IrType<'db>> {
    let type_ = match name {
        "bool" => IrType::Primitive(Primitive::Boolean),
        "char" => IrType::Primitive(Primitive::Char),
        "str" => IrType::Str,
        "i8" => IrType::Primitive(Primitive::i8),
        "i16" => IrType::Primitive(Primitive::i16),
        "i32" => IrType::Primitive(Primitive::i32),
        "u8" => IrType::Primitive(Primitive::u8),
        "u16" => IrType::Primitive(Primitive::u16),
        "u32" => IrType::Primitive(Primitive::u32),
        "f32" => IrType::Primitive(Primitive::f32),
        "f64" => IrType::Primitive(Primitive::f64),
        _ => return None,
    };

    Some(type_)
}

#[derive(Error, Diagnostic, Debug, Clone, Hash, Eq, PartialEq, salsa::Update)]
pub enum TypeLowerError {
    #[error("unsupported type: {type_:#?}")]
    #[diagnostic(code(rust_curse::unsupported_type), severity(Warning))]
    UnsupportedType { type_: Type },

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
}
