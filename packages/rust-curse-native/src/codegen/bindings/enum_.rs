use crate::{
    codegen::{
        db::Db,
        fragment::FragmentOnce,
        ir::{IrId, enum_::enum_binding_name_from_ir_id},
    },
    frag,
};

pub fn compile_binding_enum_from_tethered_expr<'db>(
    db: &'db dyn Db,
    id: IrId<'db>,
    tethered_struct_expr: impl FragmentOnce + 'db,
) -> impl FragmentOnce + 'db {
    let binding_name = enum_binding_name_from_ir_id(db, id);
    frag!({binding_name}~{{ src: {tethered_struct_expr} }})
}
