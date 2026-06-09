//! Lowers the rustdoc type to an ir's [`ir::Type`].

use rustdoc_types::Type;

use crate::codegen::{
    db::Db,
    ir,
    rustdoc::{CrateExt, RustdocType},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LoweredType {
    Moved(ir::Type),
    Borrowed { type_: ir::Type, is_mutable: bool },
}

/// Lowers the rustdoc type to an ir's [`ir::Type`].
#[salsa::tracked]
pub fn lower_type<'db>(db: &'db dyn Db, id: RustdocType<'db>) -> Option<LoweredType> {
    let type_ = id.type_(db);
    let crate_ = id.crate_(db);

    match type_ {
        Type::Primitive(name) => {
            let primitive = lower_primitive(name);
            if primitive.is_none() {
                println!("unsupported primitive: {}", name);
                return None;
            }

            primitive.map(|t| LoweredType::Moved(ir::Type::Primitive(t)))
        }
        Type::ResolvedPath(..) => {
            println!("structs are not supported");
            None
        }
        Type::BorrowedRef {
            lifetime,
            is_mutable,
            type_,
        } => {
            if lifetime.is_some() {
                println!("lifetime types are not supported");
                return None;
            }

            Some(LoweredType::Borrowed {
                is_mutable: *is_mutable,
                type_: match lower_type(db, crate_.ty(db, type_))? {
                    LoweredType::Moved(t) => t,
                    LoweredType::Borrowed { .. } => {
                        println!("nested borrowed types are not supported");
                        return None;
                    }
                },
            })
        }
        Type::Tuple(..) => {
            println!("tuple types are not supported");
            None
        }
        Type::DynTrait(..) => {
            println!("dyn trait types are not supported");
            None
        }
        Type::Generic(..) => {
            println!("generic types are not supported");
            None
        }
        Type::FunctionPointer(..) => {
            println!("function pointer types are not supported");
            None
        }
        Type::Slice(..) => {
            println!("slice types are not supported");
            None
        }
        Type::Array { .. } => {
            println!("array types are not supported");
            None
        }
        Type::Pat { .. } => {
            println!("pattern types are not supported");
            None
        }
        Type::ImplTrait(..) => {
            println!("impl trait types are not supported");
            None
        }
        Type::Infer => {
            println!("infer is not supported");
            None
        }
        Type::RawPointer { .. } => {
            println!("raw pointer types are not supported");
            None
        }
        Type::QualifiedPath { .. } => {
            println!("qualified paths are not supported");
            None
        }
    }
}

fn lower_primitive(name: &str) -> Option<ir::Primitive> {
    match name {
        "bool" => Some(ir::Primitive::Boolean),
        "char" => Some(ir::Primitive::Char),
        "str" => Some(ir::Primitive::Str),
        "i8" => Some(ir::Primitive::i8),
        "i16" => Some(ir::Primitive::i16),
        "i32" => Some(ir::Primitive::i32),
        "i64" => Some(ir::Primitive::i64),
        "u8" => Some(ir::Primitive::u8),
        "u16" => Some(ir::Primitive::u16),
        "u32" => Some(ir::Primitive::u32),
        "u64" => Some(ir::Primitive::u64),
        "isize" => Some(ir::Primitive::isize),
        "usize" => Some(ir::Primitive::usize),
        "f32" => Some(ir::Primitive::f32),
        "f64" => Some(ir::Primitive::f64),
        _ => None,
    }
}
