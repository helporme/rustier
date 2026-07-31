use crate::codegen::{
    bindings::tethering::{ClonedField, TetheredField, TetheringStructField},
    db::Db,
    ir::shape::ShapeField,
};

impl<'db> ShapeField<'db> {
    /// Converts this field to a [`TetheredField`] of the tethering struct.
    pub fn to_tethered_field<GetRefExpr: 'db, GetMutExpr: 'db>(
        &self,
        db: &'db dyn Db,
        get_ref_expr: GetRefExpr,
        get_mut_expr: GetMutExpr,
    ) -> TetheredField<'db, GetRefExpr, GetMutExpr> {
        let source_type = self.type_(db).source_type_owned();

        if source_type.is_tethered(db) {
            TetheredField::TetheringStruct(TetheringStructField {
                name: self.name(db),
                get_ref_expr,
                get_mut_expr,
                source_type,
                with_setter: !self.is_readonly(db),
            })
        } else {
            TetheredField::Cloned(ClonedField {
                name: self.name(db),
                get_ref_expr,
                get_mut_expr,
                source_type,
            })
        }
    }
}
