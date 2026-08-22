//! Module for running checks on IR for spotting common (but not critical)
//! mistakes and pitfalls when using rust-curse.

use crate::compiler::{
    db::Db,
    diag::Diag,
    ir::rust::{Crate, Member, Mod},
};

mod enum_js_name_collisions;
mod f32_precision_loss;

impl<'db> Crate<'db> {
    pub fn check(self, db: &'db dyn Db) -> Vec<&'db Diag> {
        accumulate_checks::accumulated::<Diag>(db, self)
    }
}

#[salsa::tracked]
fn accumulate_checks<'db>(db: &'db dyn Db, crate_: Crate<'db>) {
    crate_.root(db).check(db);
}

#[salsa::tracked]
impl<'db> Mod<'db> {
    #[salsa::tracked]
    fn check(self, db: &'db dyn Db) {
        for member in self.members(db) {
            match member {
                Member::Struct(struct_) => struct_.check_f32_precision_loss(db),
                Member::Enum(enum_) => {
                    enum_.check_f32_precision_loss(db);
                    enum_.check_js_name_collisions(db);
                }
                Member::Function(function) => function.check_f32_precision_loss(db),
                Member::Mod(mod_) => mod_.check(db),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Db;
    use crate::{compiler::ir::rust::Rust, test_db};

    pub(super) fn diagnostic_codes(db: &dyn Db) -> Vec<String> {
        let crate_ = Rust::get(db)
            .crates(db)
            .first()
            .expect("test crate should be lowered");

        crate_
            .check(db)
            .iter()
            .filter_map(|diag| diag.inner.code().map(|code| code.to_string()))
            .collect()
    }

    #[test]
    fn checks_nested_modules() {
        let db = test_db! {
            pub mod measurements {
                pub fn read() -> f32 {
                    1.25
                }
            }
        };

        assert_eq!(diagnostic_codes(&db), ["rust_curse::f32_precision_loss"]);
    }
}
