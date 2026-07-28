use crate::{
    codegen::bindings::{
        primitive::{
            CastCharToU32BoundaryRule, CastF32ToF64BoundaryRule, CastF64ToF32Rule,
            CastF64ToSignedIntRule, CastF64ToUnsignedIntRule, CastJsAnyToBoolRule,
            CastJsAnyToF64Rule, CastJsAnyToStringRule, CastPrimitiveToSameBoundaryRule,
            CastStringToStrMutRule, CastStringToStrRefRule, CastU32ToCharRule,
        },
        struct_::{CastStructBoundaryTypeToBorrowedSourceType, CastStructSourceTypeToBoundaryType},
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
            CharToU32Boundary {
                rule: CastCharToU32BoundaryRule,
                from: SourceType,
                to: BoundaryType,
            }
            F32ToF64Boundary {
                rule: CastF32ToF64BoundaryRule,
                from: SourceType,
                to: BoundaryType,
            }
            PrimitiveToSameBoundary {
                rule: CastPrimitiveToSameBoundaryRule,
                from: SourceType,
                to: BoundaryType,
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
            StructBoundaryTypeToBorrowedSourceType {
                rule: CastStructBoundaryTypeToBorrowedSourceType,
                from: BoundaryType,
                to: SourceType,
            }
            StructSourceTypeToBoundaryType {
                rule: CastStructSourceTypeToBoundaryType,
                from: SourceType,
                to: BoundaryType,
            }
       }
    }
}
