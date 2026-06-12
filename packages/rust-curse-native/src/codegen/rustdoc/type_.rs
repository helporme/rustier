use rustdoc_types::Type;

use crate::{
    codegen::{
        db::Db,
        ir,
        rustdoc::{CrateExt, RustdocType},
    },
    diag::{Diag, DiagResult},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LoweredType {
    Moved(ir::Type),
    Borrowed { type_: ir::Type, is_mutable: bool },
}

/// Lowers the rustdoc type.
#[salsa::tracked]
pub fn lower_type<'db>(db: &'db dyn Db, id: RustdocType<'db>) -> DiagResult<LoweredType> {
    let type_ = id.type_(db);
    let crate_ = id.crate_(db);

    match type_ {
        Type::Primitive(name) => match lower_primitive(name) {
            Some(primitive) => Ok(LoweredType::Moved(ir::Type::Primitive(primitive))),
            None => Err(vec![Diag::unsupported_primitive(name)]),
        },
        Type::BorrowedRef {
            lifetime,
            is_mutable,
            type_,
        } => {
            let mut diags = Vec::new();

            if lifetime.is_some() {
                diags.push(Diag::unsupported_lifetime_ref());
            }

            let inner = match lower_type(db, crate_.ty(db, type_)) {
                Ok(LoweredType::Moved(inner)) => Some(inner),
                Ok(LoweredType::Borrowed { .. }) => {
                    diags.push(Diag::unsupported_nested_ref());
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
        _ => Err(vec![Diag::unsupported_type(type_)]),
    }
}

fn lower_primitive(name: &str) -> Option<ir::Primitive> {
    match name {
        "bool" => Some(ir::Primitive::Boolean),
        "str" => Some(ir::Primitive::Str),
        "i8" => Some(ir::Primitive::i8),
        "i16" => Some(ir::Primitive::i16),
        "i32" => Some(ir::Primitive::i32),
        "u8" => Some(ir::Primitive::u8),
        "u16" => Some(ir::Primitive::u16),
        "u32" => Some(ir::Primitive::u32),
        "f64" => Some(ir::Primitive::f64),
        _ => None,
    }
}

impl Diag {
    pub fn unsupported_type(type_: &Type) -> Diag {
        let kind = match type_ {
            Type::Primitive(..) => "primitive",
            Type::ResolvedPath(..) => "struct",
            Type::DynTrait(..) => "dyn trait",
            Type::Generic(..) => "generic",
            Type::FunctionPointer(..) => "function pointer",
            Type::Tuple(..) => "tuple",
            Type::Slice(..) => "slice",
            Type::Array { .. } => "array",
            Type::Pat { .. } => "pattern",
            Type::ImplTrait(..) => "impl trait",
            Type::Infer => "inferred",
            Type::RawPointer { .. } => "raw pointer",
            Type::BorrowedRef { .. } => "borrowed reference",
            Type::QualifiedPath { .. } => "qualified path",
        };

        Diag::new(
            "rust-curse::unsupported_type",
            format!("{kind} types are not supported"),
        )
    }

    pub fn unsupported_primitive(name: &str) -> Diag {
        Diag::new(
            "rust-curse::unsupported_primitive",
            format!("the primitive type `{name}` is not supported"),
        )
        .with_help("supported primitive types are bool, str, i8, i16, i32, u8, u16, u32 and f64")
    }

    pub fn unsupported_lifetime_ref() -> Diag {
        Diag::new(
            "rust-curse::unsupported_lifetime_ref",
            "references with explicit lifetimes are not supported",
        )
    }

    pub fn unsupported_nested_ref() -> Diag {
        Diag::new(
            "rust-curse::unsupported_nested_ref",
            "nested references are not supported",
        )
        .with_help("use a single reference instead of a double reference")
    }
}
