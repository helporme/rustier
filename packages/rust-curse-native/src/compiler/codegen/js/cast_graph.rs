use crate::define_cast_graph;

define_cast_graph! {
    pub FacadeCast<'db> {
        types {
            FacadeType(crate::compiler::ir::facade::FacadeType<'db>),
            FacadeFfiType(crate::compiler::codegen::js::types::FacadeFfiType<'db>),
        }
        direct_rules {
            FacadeClassTypeToFfiClassType {
                rule: crate::compiler::codegen::js::classes::CastFacadeClassTypeToFfiClassTypeRule,
                from: FacadeType,
                to: FacadeFfiType,
            }
            FfiClassTypeToFacadeClassType {
                rule: crate::compiler::codegen::js::classes::CastFfiClassTypeToFacadeClassTypeRule,
                from: FacadeFfiType,
                to: FacadeType,
            }
        }
        indirect_rules  {

        }
    }
}
