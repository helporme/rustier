//! Lowers an [`super::super::asg::ASG`] into the per-target [`IR`].
//!
//! Today this is a direct clone -> at this point the IR and ASG are
//! structurally identical. The seam exists so future passes (synthesising
//! ctors, flattening impl blocks into class shapes, etc.) can live here
//! without disturbing the ASG.

use crate::codegen::asg::{
    self as asg, Ctor as AsgCtor, Export as AsgExport, FnDef as AsgFnDef, FnParam as AsgFnParam,
    Item as AsgItem, ModDef as AsgModDef, Primitive as AsgPrimitive, Span as AsgSpan,
    StructDef as AsgStructDef, StructField as AsgStructField, StructShape as AsgStructShape,
    Type as AsgType, ASG,
};
use crate::warns::Warns;

use super::{
    Ctor, Export, FnDef, FnParam, Id, Item, ItemPath, ModDef, ParamMode, Primitive, Span,
    StructDef, StructField, StructShape, Type, IR,
};

/// Errors that may be produced by [`IR::from_asg`].
///
/// Lowering is currently total: every well-formed ASG produces a valid
/// IR. The enum is reserved so future passes can surface failures without
/// changing the function signature.
#[derive(Debug, thiserror::Error)]
pub enum FromAsgError {
    /// Reserved sentinel - never constructed by today's lowering.
    #[error("unreachable")]
    Unreachable,
}

impl IR {
    /// Lower an [`ASG`] into the per-target [`IR`].
    ///
    /// `warns` is accepted so future lowering passes can record
    /// non-fatal diagnostics; today no warnings are emitted.
    pub fn from_asg(warns: &mut Warns, asg: &ASG) -> Result<IR, FromAsgError> {
        let _ = warns;

        let items = asg
            .items
            .iter()
            .map(lower_item)
            .collect::<Vec<_>>();

        Ok(IR {
            root: lower_id(asg.root),
            crate_name: asg.crate_name.clone(),
            items,
        })
    }
}

fn lower_id(id: asg::Id) -> Id {
    Id(id.0)
}

fn lower_item(item: &AsgItem) -> Item {
    match item {
        AsgItem::Struct(def) => Item::Struct(lower_struct(def)),
        AsgItem::Fn(def) => Item::Fn(lower_fn(def)),
        AsgItem::Mod(def) => Item::Mod(lower_mod(def)),
    }
}

fn lower_struct(def: &AsgStructDef) -> StructDef {
    StructDef {
        path: lower_path(&def.path),
        span: def.span.as_ref().map(lower_span),
        ctor: lower_ctor(&def.ctor),
        clonable: def.clonable,
        shape: lower_shape(&def.shape),
        docs: def.docs.clone(),
    }
}

fn lower_fn(def: &AsgFnDef) -> FnDef {
    FnDef {
        path: lower_path(&def.path),
        span: def.span.as_ref().map(lower_span),
        params: def.params.iter().map(lower_param).collect(),
        return_type: lower_type(&def.return_type),
        docs: def.docs.clone(),
    }
}

fn lower_mod(def: &AsgModDef) -> ModDef {
    ModDef {
        path: lower_path(&def.path),
        span: def.span.as_ref().map(lower_span),
        exports: def.exports.iter().map(lower_export).collect(),
    }
}

fn lower_path(p: &asg::ItemPath) -> ItemPath {
    ItemPath::new(p.segments.clone())
}

fn lower_span(s: &AsgSpan) -> Span {
    Span {
        filename: s.filename.clone(),
        begin: s.begin,
        end: s.end,
    }
}

fn lower_ctor(c: &AsgCtor) -> Ctor {
    match c {
        AsgCtor::Explicit(id) => Ctor::Explicit(lower_id(*id)),
        AsgCtor::Implicit => Ctor::Implicit,
    }
}

fn lower_shape(s: &AsgStructShape) -> StructShape {
    match s {
        AsgStructShape::Unit => StructShape::Unit,
        AsgStructShape::Tuple(ts) => StructShape::Tuple(ts.iter().map(lower_type).collect()),
        AsgStructShape::Struct(fs) => StructShape::Struct(fs.iter().map(lower_field).collect()),
    }
}

fn lower_field(f: &AsgStructField) -> StructField {
    StructField {
        name: f.name.clone(),
        ty: lower_type(&f.ty),
        docs: f.docs.clone(),
    }
}

fn lower_param(p: &AsgFnParam) -> FnParam {
    FnParam {
        name: p.name.clone(),
        mode: match p.mode {
            asg::ParamMode::ByValue => ParamMode::ByValue,
            asg::ParamMode::ByRef => ParamMode::ByRef,
            asg::ParamMode::ByMut => ParamMode::ByMut,
        },
        ty: lower_type(&p.ty),
    }
}

fn lower_type(ty: &AsgType) -> Type {
    match ty {
        AsgType::Primitive(p) => Type::Primitive(lower_primitive(p)),
        AsgType::Struct(id) => Type::Struct(lower_id(*id)),
    }
}

fn lower_primitive(p: &AsgPrimitive) -> Primitive {
    match p {
        AsgPrimitive::Unit => Primitive::Unit,
        AsgPrimitive::Boolean => Primitive::Boolean,
        AsgPrimitive::Char => Primitive::Char,
        AsgPrimitive::String => Primitive::String,
        AsgPrimitive::Str => Primitive::Str,
        AsgPrimitive::i8 => Primitive::i8,
        AsgPrimitive::u8 => Primitive::u8,
        AsgPrimitive::i16 => Primitive::i16,
        AsgPrimitive::u16 => Primitive::u16,
        AsgPrimitive::i32 => Primitive::i32,
        AsgPrimitive::u32 => Primitive::u32,
        AsgPrimitive::i64 => Primitive::i64,
        AsgPrimitive::u64 => Primitive::u64,
        AsgPrimitive::isize => Primitive::isize,
        AsgPrimitive::usize => Primitive::usize,
        AsgPrimitive::f32 => Primitive::f32,
        AsgPrimitive::f64 => Primitive::f64,
    }
}

fn lower_export(e: &AsgExport) -> Export {
    Export {
        name: e.name.clone(),
        target: lower_id(e.target),
    }
}
