use crate::{
    codegen::bindings::{
        primitive::{
            CastF64ToF32Rule, CastF64ToSignedIntRule, CastF64ToUnsignedIntRule,
            CastJsAnyToBoolRule, CastJsAnyToF64Rule, CastJsAnyToStringRule, CastStringToStrMutRule,
            CastStringToStrRefRule, CastU32ToCharRule,
        },
        type_::{BoundaryType, CastOwnedTypeToMutTypeRule, CastOwnedTypeToRefTypeRule, SourceType},
    },
    define_cast_graph,
};

define_cast_graph! {
    pub BindingsCastGraph<'db> {
        types {
            BoundaryType(BoundaryType<'db>),
            SourceType(SourceType<'db>)
        }
        rules {
            OwnedTypeToRefType {
                rule: CastOwnedTypeToRefTypeRule,
                from: SourceType,
                to: SourceType,
            }
            OwnedTypeToMutType {
                rule: CastOwnedTypeToMutTypeRule,
                from: SourceType,
                to: SourceType,
            }
            JsAnyToF64 {
                rule: CastJsAnyToF64Rule,
                from: BoundaryType,
                to: SourceType,
            }
            JsAnyToBool {
                rule: CastJsAnyToBoolRule,
                from: BoundaryType,
                to: SourceType,
            }
            JsAnyToString {
                rule: CastJsAnyToStringRule,
                from: BoundaryType,
                to: SourceType,
            }
            F64ToSignedInt {
                rule: CastF64ToSignedIntRule,
                from: SourceType,
                to: SourceType,
            }
            F64ToUnsignedInt {
                rule: CastF64ToUnsignedIntRule,
                from: SourceType,
                to: SourceType,
            }
            U32ToChar {
                rule: CastU32ToCharRule,
                from: SourceType,
                to: SourceType,
            }
            F64ToF32 {
                rule: CastF64ToF32Rule,
                from: SourceType,
                to: SourceType,
            }
            StringToStrRef {
                rule: CastStringToStrRefRule,
                from: SourceType,
                to: SourceType,
            }
            StringToStrMut {
                rule: CastStringToStrMutRule,
                from: SourceType,
                to: SourceType,
            }
        }
    }
}
