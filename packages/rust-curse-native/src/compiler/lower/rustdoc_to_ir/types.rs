use miette::Diagnostic;
use rustdoc_types::{ItemKind as RItemKind, Type};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    ir::{EnumId, StructId, Type as IrType, ValueType as IrValueType},
    ir::{ItemId, ItemKind, ItemPath},
    rustdoc::{CrateExt, RustdocType},
};

/// Lowers the rustdoc type to the IR type.
pub fn lower_type<'db>(
    db: &'db dyn Db,
    id: RustdocType<'db>,
) -> Result<IrType<'db>, Vec<TypeLowerError>> {
    let type_ = id.type_(db);
    let crate_ = id.crate_(db);

    match type_ {
        Type::Primitive(name) => match lower_primitive(name) {
            Some(type_) => Ok(IrType::Value(type_)),
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
                Ok(IrType::Value(IrValueType::String))
            } else if summary.kind == RItemKind::Struct && crate_.index.contains_key(&path.id) {
                Ok(IrType::Value(IrValueType::Struct(
                    StructId::new(
                        db,
                        ItemId::new(db, ItemPath::new(db, &summary.path), ItemKind::Struct),
                    )
                    .expect("newly created struct item must have struct kind"),
                )))
            } else if summary.kind == RItemKind::Enum && crate_.index.contains_key(&path.id) {
                Ok(IrType::Value(IrValueType::Enum(
                    EnumId::new(
                        db,
                        ItemId::new(db, ItemPath::new(db, &summary.path), ItemKind::Enum),
                    )
                    .expect("newly created enum item must have enum kind"),
                )))
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
                Ok(IrType::Value(inner)) => Some(inner),
                Ok(IrType::Borrowed { .. }) => {
                    diags.push(TypeLowerError::UnsupportedDoubleReference);
                    None
                }
                Err(inner_diags) => {
                    diags.extend(inner_diags);
                    None
                }
            };

            match inner {
                Some(inner) if diags.is_empty() => Ok(IrType::Borrowed {
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

fn lower_primitive<'db>(name: &str) -> Option<IrValueType<'db>> {
    let type_ = match name {
        "bool" => IrValueType::bool,
        "char" => IrValueType::char,
        "str" => IrValueType::Str,
        "i8" => IrValueType::i8,
        "i16" => IrValueType::i16,
        "i32" => IrValueType::i32,
        "u8" => IrValueType::u8,
        "u16" => IrValueType::u16,
        "u32" => IrValueType::u32,
        "f32" => IrValueType::f32,
        "f64" => IrValueType::f64,
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
