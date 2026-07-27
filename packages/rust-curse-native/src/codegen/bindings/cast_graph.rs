use crate::{
    codegen::bindings::{
        primitive::{
            CastBoundaryJsTypeToBoolRule, CastBoundaryJsTypeToF64Rule,
            CastBoundaryJsTypeToStringRule, CastF64ToF32Rule, CastF64ToSignedIntRule,
            CastF64ToUnsignedIntRule, CastStringToStrMutRule, CastStringToStrRefRule,
            CastU32ToCharRule,
        },
        type_::{CastOwnedTypeToMutTypeRule, CastOwnedTypeToRefTypeRule, JsType, SourceType},
    },
    define_cast_graph,
};

define_cast_graph! {
    pub BindingsCastGraph<'db> {
        types {
            BoundaryJsType(JsType),
            BoundaryType(SourceType<'db>)
        }
        rules {
            OwnedTypeToRefType {
                rule: CastOwnedTypeToRefTypeRule,
                from: BoundaryType,
                to: BoundaryType,
            }
            OwnedTypeToMutType {
                rule: CastOwnedTypeToMutTypeRule,
                from: BoundaryType,
                to: BoundaryType,
            }
            BoundaryJsTypeToF64 {
                rule: CastBoundaryJsTypeToF64Rule,
                from: BoundaryJsType,
                to: BoundaryType,
            }
            BoundaryJsTypeToBool {
                rule: CastBoundaryJsTypeToBoolRule,
                from: BoundaryJsType,
                to: BoundaryType,
            }
            BoundaryJsTypeToString {
                rule: CastBoundaryJsTypeToStringRule,
                from: BoundaryJsType,
                to: BoundaryType,
            }
            F64ToSignedInt {
                rule: CastF64ToSignedIntRule,
                from: BoundaryType,
                to: BoundaryType,
            }
            F64ToUnsignedInt {
                rule: CastF64ToUnsignedIntRule,
                from: BoundaryType,
                to: BoundaryType,
            }
            U32ToChar {
                rule: CastU32ToCharRule,
                from: BoundaryType,
                to: BoundaryType,
            }
            F64ToF32 {
                rule: CastF64ToF32Rule,
                from: BoundaryType,
                to: BoundaryType,
            }
            StringToStrRef {
                rule: CastStringToStrRefRule,
                from: BoundaryType,
                to: BoundaryType,
            }
            StringToStrMut {
                rule: CastStringToStrMutRule,
                from: BoundaryType,
                to: BoundaryType,
            }
        }
    }
}
