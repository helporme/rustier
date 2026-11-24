//! Analysis running on macros to derive it's input/output types.

use ra_ap_hir_def::builtin_type::{BuiltinFloat, BuiltinInt, BuiltinType, BuiltinUint};
use ra_ap_hir_ty::next_solver::{GenericArgKind, Ty, TyKind};

use super::{BigIntType, HandleBorrow, RsType};
use crate::{db::Db, ir::rs::RsProgram};

mod compile_errors;
mod js_macros;
mod rs_macros;

pub use compile_errors::CompileError;

fn classify_type(db: &dyn Db, program: RsProgram, ty: Ty<'_>) -> RsType {
    if let Some((inner, _, mutability)) = ty.as_reference() {
        return match classify_type(db, program, inner) {
            RsType::RustHandle {
                borrow: HandleBorrow::Owned,
            } => RsType::RustHandle {
                borrow: if mutability.is_mut() {
                    HandleBorrow::Mutable
                } else {
                    HandleBorrow::Shared
                },
            },
            _ => RsType::JsValue,
        };
    }

    if ty.is_unit() {
        RsType::Void
    } else if ty.as_adt().is_some_and(|(adt, _)| {
        let ra_ap_hir_def::AdtId::StructId(id) = adt else {
            return false;
        };
        let struct_: ra_ap_hir::Struct = id.into();
        let module = struct_.module(db);
        let crate_ = module.krate(db);

        crate_.is_builtin(db)
            && struct_.name(db).display(db, crate_.edition(db)).to_string() == "String"
            && module
                .path_segments(db)
                .map(|name| name.display(db, crate_.edition(db)).to_string())
                .eq(["string"])
    }) {
        RsType::String
    } else {
        match ty.as_builtin() {
            Some(BuiltinType::Bool) => RsType::Bool,
            Some(BuiltinType::Int(BuiltinInt::I8)) => RsType::I8,
            Some(BuiltinType::Int(BuiltinInt::I16)) => RsType::I16,
            Some(BuiltinType::Int(BuiltinInt::I32)) => RsType::I32,
            Some(BuiltinType::Uint(BuiltinUint::U8)) => RsType::U8,
            Some(BuiltinType::Uint(BuiltinUint::U16)) => RsType::U16,
            Some(BuiltinType::Uint(BuiltinUint::U32)) => RsType::U32,
            Some(BuiltinType::Uint(BuiltinUint::Usize)) => RsType::BigInt(BigIntType::Usize),
            Some(BuiltinType::Int(BuiltinInt::I64)) => RsType::BigInt(BigIntType::I64),
            Some(BuiltinType::Uint(BuiltinUint::U64)) => RsType::BigInt(BigIntType::U64),
            Some(BuiltinType::Int(BuiltinInt::I128)) => RsType::BigInt(BigIntType::I128),
            Some(BuiltinType::Uint(BuiltinUint::U128)) => RsType::BigInt(BigIntType::U128),
            Some(BuiltinType::Float(BuiltinFloat::F32)) => RsType::F32,
            Some(BuiltinType::Float(BuiltinFloat::F64)) => RsType::F64,
            _ => classify_owned_handle(db, program, ty),
        }
    }
}

fn classify_owned_handle(db: &dyn Db, program: RsProgram, ty: Ty<'_>) -> RsType {
    if ty.references_non_lt_error()
        || ty
            .walk()
            .any(|arg| matches!(arg.kind(), GenericArgKind::Lifetime(_)))
        || matches!(
            ty.inner().internee,
            TyKind::Never | TyKind::Str | TyKind::Slice(_)
        )
    {
        return RsType::JsValue;
    }

    if program.is_js_value(db, ty) {
        return RsType::JsValue;
    }

    RsType::RustHandle {
        borrow: HandleBorrow::Owned,
    }
}
