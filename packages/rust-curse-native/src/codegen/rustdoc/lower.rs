use super::*;
use crate::codegen::{asg, db::Db};
use rustdoc_types::{Abi, ItemEnum, Span, StructKind, Type, VariantKind, Visibility};

#[salsa::tracked]
pub fn lower_crate(db: &dyn Db, crate_: CrateDef) -> asg::Asg<'_> {
    lower_mod(db, crate_, crate_.value(db).root)
}

// todo: id is ustable, replace two params with ItemPtr which will contain
// craete and stable ptr
#[salsa::tracked]
pub fn lower_item(db: &dyn Db, crate_: CrateDef, id: Id) -> Option<asg::Item<'_>> {
    let item = crate_.item(db, id);
    match &item.inner {
        ItemEnum::Function(..) => Some(asg::Item::Fn(lower_fn(db, crate_, id)?)),
        ItemEnum::Struct(..) => Some(asg::Item::Struct(lower_struct(db, crate_, id)?)),
        ItemEnum::Trait(..) => Some(asg::Item::Trait(lower_trait(db, crate_, id)?)),
        ItemEnum::Enum(..) => Some(asg::Item::Enum(lower_enum(db, crate_, id)?)),
        ItemEnum::Module(..) => Some(asg::Item::Mod(lower_mod(db, crate_, id))),
        _ => None,
    }
}

#[salsa::tracked]
pub fn lower_mod(db: &dyn Db, crate_: CrateDef, id: Id) -> asg::ModDef<'_> {
    let item = crate_.item(db, id);
    let def = match &item.inner {
        ItemEnum::Module(def) => def,
        _ => unreachable!(),
    };

    let path = lower_path(db, crate_, id);
    let span = item.span.clone().map(|s| s.into());

    let mut items = Vec::new();
    let mut uses = Vec::new();

    for id in &def.items {
        if let Some(item) = lower_item(db, crate_, *id) {
            items.push(item);
        } else if matches!(crate_.item(db, *id).inner, ItemEnum::Use(..)) {
            if let Some(use_) = lower_use(db, crate_, *id) {
                uses.push(use_);
            }
        }
    }

    asg::ModDef::new(db, path, span, uses, items)
}

#[salsa::tracked]
pub fn lower_use(db: &dyn Db, crate_: CrateDef, id: Id) -> Option<asg::Use<'_>> {
    let item = crate_.item(db, id);
    let def = match &item.inner {
        ItemEnum::Use(def) => def,
        _ => unreachable!(),
    };

    if def.is_glob {
        println!("use mod::* are not supported");
        return None;
    }

    let item = lower_item(db, crate_, def.id?)?;

    Some(asg::Use {
        name: def.name.clone(),
        item,
    })
}

#[salsa::tracked]
pub fn lower_fn(db: &dyn Db, crate_: CrateDef, id: Id) -> Option<asg::FnDef<'_>> {
    let item = crate_.item(db, id);

    let def = match &item.inner {
        ItemEnum::Function(def) => def,
        _ => unreachable!(),
    };

    if def.has_body && item.visibility != Visibility::Public {
        return None;
    }

    // todo: warn about all of this at once in case it's used

    if def.header.is_async {
        // todo: warning
        println!("async functions are not supported");
        return None;
    }

    if def.header.is_unsafe {
        // todo: warning
        println!("unsafe functions are not supported");
        return None;
    }

    if def.header.abi != Abi::Rust {
        // todo: warning
        println!("{:?} functions are not supported", def.header.abi);
        return None;
    }

    if def.sig.is_c_variadic {
        // todo: warning
        println!("c-variadic functions are not supported");
        return None;
    }

    if !def.generics.params.is_empty() {
        // todo: warning
        println!("generic functions are not supported");
        return None;
    }

    let mut params = Vec::new();

    for (name, type_) in &def.sig.inputs {
        let type_ = lower_type(db, crate_, type_.clone());

        if type_.is_none() {
            println!("function is skipped since param {name} is not supported");
            return None;
        }

        params.push(asg::FnParam {
            name: name.clone(),
            ty: type_.unwrap(),
        });
    }

    let ret_type = def
        .sig
        .output
        .as_ref()
        .and_then(|type_| lower_type(db, crate_, type_.clone()));

    let path = lower_path(db, crate_, id);
    let span = item.span.clone().map(|s| s.into());

    Some(asg::FnDef::new(
        db,
        path,
        def.has_body,
        span,
        params,
        ret_type,
        item.docs.clone(),
    ))
}

#[salsa::tracked]
pub fn lower_struct(db: &dyn Db, crate_: CrateDef, id: Id) -> Option<asg::StructDef<'_>> {
    let item = crate_.item(db, id);

    let def = match &item.inner {
        ItemEnum::Struct(def) => def,
        _ => unreachable!(),
    };

    if !def.generics.params.is_empty() {
        println!("generics are unsupported on a struct");
        return None;
    }

    let shape = match &def.kind {
        StructKind::Plain {
            fields,
            has_stripped_fields,
        } => {
            let mut shape_fields = Vec::new();
            let mut has_stripped_fields = *has_stripped_fields;

            for id in fields {
                if let Some(field) = lower_struct_field(db, crate_, *id) {
                    shape_fields.push(field);
                } else {
                    has_stripped_fields = true;
                }
            }

            asg::StructShape::Plain {
                fields: shape_fields,
                has_stripped_fields,
            }
        }
        StructKind::Unit => {
            println!("unit structs or enum variants are unsupported");
            return None;
        }
        StructKind::Tuple(..) => {
            println!("tuple structs or enum variants are unsupported");
            return None;
        }
    };

    let path = lower_path(db, crate_, id);
    let span = item.span.clone().map(|s| s.into());

    Some(asg::StructDef::new(
        db,
        path,
        span,
        shape,
        Vec::new(), // todo: impls
        item.docs.clone(),
    ))
}

fn lower_struct_field(db: &dyn Db, crate_: CrateDef, id: Id) -> Option<asg::StructField<'_>> {
    let item = crate_.item(db, id);
    let ty = match &item.inner {
        ItemEnum::StructField(ty) => ty,
        _ => unreachable!(),
    };

    let lowered_ty = lower_type(db, crate_, ty.clone());

    if lowered_ty.is_none() {
        println!("filed is using unsupported type");
        return None;
    }

    Some(asg::StructField {
        name: item.name.clone().unwrap(),
        ty: lowered_ty.unwrap(),
        span: item.span.clone().map(|s| s.into()),
        docs: item.docs.clone(),
    })
}

#[salsa::tracked]
pub fn lower_impl(db: &dyn Db, crate_: CrateDef, id: Id) -> Option<asg::ImplDef<'_>> {
    let item = crate_.item(db, id);

    let def = match &item.inner {
        ItemEnum::Impl(def) => def,
        _ => unreachable!(),
    };

    if def.is_synthetic | def.is_negative {
        return None;
    }

    if !def.generics.params.is_empty() {
        println!("generics are not supported");
        return None;
    }

    if def.is_unsafe {
        println!("unsafe impls are not supported");
        return None;
    }

    if !def.provided_trait_methods.is_empty() {
        println!("provided trait methods are not supported");
        // we are fine with ignoring them
    }

    let for_id = match &def.for_ {
        Type::ResolvedPath(path) => lower_path(db, crate_, path.id),
        _ => {
            println!("unsupported impl for type: {:?}", def.for_);
            return None;
        }
    };

    let trait_id = def
        .trait_
        .as_ref()
        .map(|path| lower_path(db, crate_, path.id));

    let mut fns = Vec::new();
    for id in &def.items {
        if let Some(def) = lower_fn(db, crate_, *id) {
            fns.push(def);
        }
    }

    Some(asg::ImplDef::new(db, for_id, trait_id, fns))
}

#[salsa::tracked]
pub fn lower_trait(db: &dyn Db, crate_: CrateDef, id: Id) -> Option<asg::TraitDef<'_>> {
    let item = crate_.item(db, id);

    let def = match &item.inner {
        ItemEnum::Trait(def) => def,
        _ => unreachable!(),
    };

    if !def.generics.params.is_empty() {
        println!("generics are not supported");
        return None;
    }

    if !def.is_unsafe {
        println!("unsafe trait methods are not supported");
        return None;
    }

    if def.is_auto {
        println!("auto trait methods are not supported");
        return None;
    }

    let path = lower_path(db, crate_, id);
    let span = item.span.clone().map(|s| s.into());

    let mut fns = Vec::new();

    for item in &def.items {
        if let Some(fn_) = lower_fn(db, crate_, *item) {
            fns.push(fn_);
        }
    }

    let mut impls = Vec::new();

    for item in &def.implementations {
        if let Some(impl_) = lower_impl(db, crate_, *item) {
            impls.push(impl_);
        }
    }

    Some(asg::TraitDef::new(
        db,
        path,
        span,
        fns,
        impls,
        item.docs.clone(),
    ))
}

#[salsa::tracked]
pub fn lower_enum(db: &dyn Db, crate_: CrateDef, id: Id) -> Option<asg::EnumDef<'_>> {
    let item = crate_.item(db, id);

    let def = match &item.inner {
        ItemEnum::Enum(def) => def,
        _ => unreachable!(),
    };

    if def.has_stripped_variants {
        println!("enums with stripped variants are not supported");
        return None;
    }

    if !def.generics.params.is_empty() {
        println!("generics are not supported");
        return None;
    }

    let mut impls = Vec::new();
    for item in &def.impls {
        if let Some(impl_) = lower_impl(db, crate_, *item) {
            impls.push(impl_);
        }
    }

    let mut variants = Vec::new();
    for variant in &def.variants {
        if let Some(variant) = lower_enum_variant(db, crate_, *variant) {
            variants.push(variant);
        }
    }

    let path = lower_path(db, crate_, id);
    let span = item.span.clone().map(|s| s.into());

    Some(asg::EnumDef::new(
        db,
        path,
        span,
        variants,
        impls,
        item.docs.clone(),
    ))
}

fn lower_enum_variant(db: &dyn Db, crate_: CrateDef, variant: Id) -> Option<asg::EnumVariant<'_>> {
    let item = crate_.item(db, variant);

    let def = match &item.inner {
        ItemEnum::Variant(def) => def,
        _ => unreachable!(),
    };

    if def.discriminant.is_some() {
        println!("variants with discriminants are not supported");
        return None;
    }

    match &def.kind {
        VariantKind::Plain => {
            println!("plain variants are not supported");
            None
        }
        VariantKind::Tuple(..) => {
            println!("tuple variants are not supported");
            None
        }
        VariantKind::Struct {
            fields,
            has_stripped_fields,
        } => {
            let mut has_stripped_fields = *has_stripped_fields;
            let mut lowered_fields = Vec::new();

            for id in fields {
                if let Some(field) = lower_struct_field(db, crate_, *id) {
                    lowered_fields.push(field);
                } else {
                    has_stripped_fields = true;
                }
            }

            let shape = asg::StructShape::Plain {
                fields: lowered_fields,
                has_stripped_fields,
            };

            Some(asg::EnumVariant {
                name: item.name.clone().expect("variant name is missing"),
                shape,
                span: item.span.clone().map(|s| s.into()),
                docs: item.docs.clone(),
            })
        }
    }
}

#[salsa::tracked]
pub fn lower_type(db: &dyn Db, crate_: CrateDef, type_: Type) -> Option<asg::Type<'_>> {
    match type_ {
        Type::Primitive(name) => {
            let primitive = lower_primitive(&name);
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
            let item = lower_item(db, crate_, path.id)?;
            Some(match item {
                asg::Item::Struct(def) => asg::Type::Struct(def),
                asg::Item::Enum(def) => asg::Type::Enum(def),
                _ => unreachable!(),
            })
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
                is_mutable,
                ty: Box::new(lower_type(db, crate_, *type_)?),
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

#[salsa::tracked]
fn lower_path<'db>(db: &'db dyn Db, crate_: CrateDef, id: Id) -> asg::Path<'db> {
    // todo: test that all paths are correctly resolved
    let path = crate_
        .value(db)
        .paths
        .get(&id)
        .expect("no path found")
        .path
        .join("::");

    asg::Path::new(db, path)
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

impl From<Span> for asg::Span {
    fn from(span: Span) -> Self {
        asg::Span {
            filename: span.filename,
            begin: span.begin,
            end: span.end,
        }
    }
}
