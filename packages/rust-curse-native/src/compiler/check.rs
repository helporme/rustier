//! Module for running checks on IR for spotting common (but not critical)
//! mistakes and pitfalls when using rust-curse.

use crate::compiler::{
    db::Db,
    diag::Diag,
    ir::{
        rust::Type,
        shim::{Shim, ShimMember, ShimModule},
    },
};

mod coerce_name_collisions;
mod coerce_requires_clone;
mod enum_js_name_collisions;
mod f32_precision_loss;
mod method_field_collisions;

impl<'db> Shim<'db> {
    /// Collect diagnostics from lowering and checking reachable local modules.
    pub fn check(self, db: &'db dyn Db) -> Vec<&'db Diag> {
        self.crates(db)
            .filter(|crate_| crate_.is_local(db))
            .flat_map(|crate_| crate_.all_modules(db))
            .flat_map(|module| accumulate_checks::accumulated::<Diag>(db, module))
            .collect()
    }
}

#[salsa::tracked]
fn accumulate_checks<'db>(db: &'db dyn Db, module: ShimModule<'db>) {
    for member in module.src(db).members(db) {
        match member.shim_reachable_from(db, module.usages(db)) {
            ShimMember::Class(class) => {
                coerce_name_collisions::check(db, class);
                coerce_requires_clone::check(db, class);
                match class.src_type(db) {
                    Type::Struct(struct_) => {
                        struct_.check_f32_precision_loss(db);
                        struct_.check_method_field_collisions(db);
                    }
                    Type::Enum(enum_) => {
                        enum_.check_f32_precision_loss(db);
                        enum_.check_js_name_collisions(db);
                        enum_.check_method_field_collisions(db);
                    }
                    _ => unreachable!("shim classes represent structs or enums"),
                }
                for method in class.methods(db) {
                    method.src(db).check_f32_precision_loss(db);
                }
            }
            ShimMember::Function(function) => function.src(db).check_f32_precision_loss(db),
            ShimMember::Module(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Db;
    use crate::{
        compiler::ir::rust::{Rust, usages::UsageGraph},
        diagnostic_test_db as test_db,
    };

    /// Diagnostics visit local crates only; codegen fixtures need not opt in.
    #[macro_export]
    macro_rules! diagnostic_test_db {
        ($($source:tt)*) => {{
            let mut db = $crate::test_db! { $($source)* };
            $crate::compiler::inputs::LocalCrates::set(
                &mut db,
                [$crate::compiler::path::FilePath::new_virtual("/src/lib.rs")
                    .expect("valid fixture root")],
            );
            db
        }};
    }

    pub(super) fn diagnostic_codes(db: &dyn Db) -> Vec<String> {
        ra_ap_hir::attach_db(db, || {
            let crate_ = Rust::get(db)
                .crates(db)
                .first()
                .expect("test crate should be lowered");

            Rust::get(db)
                .shim_reachable_from(db, UsageGraph::new(db, [crate_.root(db)]))
                .check(db)
                .iter()
                .filter_map(|diag| diag.inner.code().map(|code| code.to_string()))
                .collect()
        })
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
