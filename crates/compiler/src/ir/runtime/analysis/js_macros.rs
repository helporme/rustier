use super::classify_type;
use crate::{
    db::Db,
    ir::{
        rs::JsMacroSrc,
        runtime::{HandleBorrow, JsMacro, RsType, Runtime},
    },
};

#[salsa::tracked]
impl<'db> Runtime<'db> {
    /// Lowers a source occurrence to its shared JS implementation.
    #[salsa::tracked]
    pub fn lower_js(self, db: &'db dyn Db, src: JsMacroSrc<'db>) -> Option<JsMacro<'db>> {
        let parsed = src.parse(db)?;
        Some(JsMacro::new(db, parsed.body, parsed.inputs.len()))
    }

    /// Classifies each interpolation at its Rust call site, not at the shared implementation.
    #[salsa::tracked(returns(ref))]
    pub fn js_input_types(self, db: &'db dyn Db, src: JsMacroSrc<'db>) -> Option<Vec<RsType>> {
        let program = self.rs_program(db);
        Some(
            src.inferred_inputs(db)?
                .into_iter()
                .map(|ty| match classify_type(db, program, ty) {
                    RsType::JsValue if !program.is_js_value(db, ty) => RsType::RustHandle {
                        borrow: HandleBorrow::Owned,
                    },
                    ty => ty,
                })
                .collect(),
        )
    }
}

#[salsa::tracked]
impl<'db> JsMacro<'db> {
    /// Whether the JavaScript body returns a value, excluding returns in nested functions.
    #[salsa::tracked]
    pub fn ret_type(self, db: &'db dyn Db) -> RsType {
        if rustier_js_syntax::returns_value(self.body(db)) {
            RsType::JsValue
        } else {
            RsType::Void
        }
    }
}
