use std::collections::BTreeSet;

use crate::{
    codegen::{
        db::Db,
        fragment::FragmentOnce,
        ir::{IrId, enum_::enum_name_from_ir_id},
        js::imports::{Import, ImportFrom},
    },
    frag,
};

/// Compiles an expression that wraps a binding class in its enum facade class.
///
/// Requires imports added by
/// [`compile_facade_class_from_binding_expr_required_imports`] to be in scope.
pub fn compile_facade_class_from_binding_expr<'db>(
    db: &'db dyn Db,
    enum_id: IrId<'db>,
    binding_class_expr: impl FragmentOnce,
) -> impl FragmentOnce {
    frag!({ enum_name_from_ir_id(db, enum_id) }.__from_binding({ binding_class_expr }))
}

/// Adds imports required by [`compile_facade_class_from_binding_expr`].
pub fn compile_facade_class_from_binding_expr_required_imports<'db>(
    db: &'db dyn Db,
    active_mod_id: IrId<'db>,
    enum_id: IrId<'db>,
    imports: &mut BTreeSet<Import<'db>>,
) {
    let enum_mod_id = enum_id.get_parent_mod_id(db).unwrap();

    if active_mod_id == enum_mod_id {
        return;
    }

    imports.insert(Import {
        item: enum_name_from_ir_id(db, enum_id),
        from: ImportFrom::RelativeToMod {
            from: active_mod_id,
            to: enum_mod_id,
        },
    });
}

/// Compiles an expression that unwraps an enum facade class into its binding
/// class.
pub fn compile_binding_class_from_facade_expr(
    facade_class_expr: impl FragmentOnce,
) -> impl FragmentOnce {
    frag!({ facade_class_expr }.__get_binding())
}
