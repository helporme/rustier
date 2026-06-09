use rustdoc_types::Type;

use crate::codegen::{
    asg,
    db::Db,
    rustdoc::{lower_item, CrateExt, RustdocType},
};

#[salsa::tracked]
pub fn lower_type<'db>(db: &'db dyn Db, id: RustdocType<'db>) -> Option<asg::Type<'db>> {
    let crate_ = id.crate_(db);
    let type_ = id.type_(db);

    match type_ {
        Type::Primitive(name) => {
            let primitive = lower_primitive(name);
            if primitive.is_none() {
                println!("unsupported primitive: {}", name);
                return None;
            }

            primitive.map(asg::Type::Primitive)
        }
        Type::ResolvedPath(path) => {
            if path.args.is_some() {
                println!("generic types are not supported");
                return None;
            }

            let item = lower_item(db, crate_.id(db, path.id))?;
            match item {
                asg::Item::Struct(def) => Some(asg::Type::Struct(def)),
                asg::Item::Enum(def) => Some(asg::Type::Enum(def)),
                _ => None,
            }
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

            Some(asg::Type::Ref {
                is_mutable: *is_mutable,
                ty: Box::new(lower_type(db, crate_.ty(db, type_))?),
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

fn lower_primitive(name: &str) -> Option<asg::Primitive> {
    match name {
        "bool" => Some(asg::Primitive::Boolean),
        "char" => Some(asg::Primitive::Char),
        "str" => Some(asg::Primitive::Str),
        "i8" => Some(asg::Primitive::i8),
        "i16" => Some(asg::Primitive::i16),
        "i32" => Some(asg::Primitive::i32),
        "i64" => Some(asg::Primitive::i64),
        "u8" => Some(asg::Primitive::u8),
        "u16" => Some(asg::Primitive::u16),
        "u32" => Some(asg::Primitive::u32),
        "u64" => Some(asg::Primitive::u64),
        "isize" => Some(asg::Primitive::isize),
        "usize" => Some(asg::Primitive::usize),
        "f32" => Some(asg::Primitive::f32),
        "f64" => Some(asg::Primitive::f64),
        _ => None,
    }
}
