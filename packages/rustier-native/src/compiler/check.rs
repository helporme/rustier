//! Module for running checks on IR for spotting common (but not critical)
//! mistakes and pitfalls when using rustier.

use itertools::Either;

use crate::compiler::{
    db::Db,
    diag::Diag,
    ir::shim::{Shim, ShimClass, ShimMember, ShimModule},
};

mod coerce_name_collisions;
mod coerce_requires_clone;
mod enum_js_name_collisions;
mod f32_precision_loss;
#[cfg(test)]
mod method_field_collisions;
mod mutable_reference_ignored;

impl<'db> Shim<'db> {
    /// Collect diagnostics from lowering and checking reachable local modules.
    pub fn check(self, db: &'db dyn Db) -> Vec<&'db Diag> {
        accumulate_checks::accumulated::<Diag>(db, self)
    }
}

#[salsa::tracked]
fn accumulate_checks<'db>(db: &'db dyn Db, shim: Shim<'db>) {
    for crate_ in shim.crates(db).filter(|crate_| crate_.is_local(db)) {
        for module in crate_.all_modules(db) {
            check_module(db, module);
        }
    }
}

#[salsa::tracked]
fn check_module<'db>(db: &'db dyn Db, module: ShimModule<'db>) {
    for member in module.members(db) {
        let classes = match member {
            ShimMember::Class(class) => Either::Left(std::iter::once(class)),
            ShimMember::Template(template) => Either::Right(template.classes(db)),
            ShimMember::Function(function) => {
                function.src(db).check_f32_precision_loss(db);
                function.src(db).check_mutable_reference_ignored(db);

                continue;
            }
            ShimMember::Module(_) => continue,
        };

        for class in classes {
            let _ = class.fields(db);

            match class {
                ShimClass::Plain(class) => {
                    let struct_ = class.src(db);
                    struct_.check_coerce_name_collisions(db);
                    struct_.check_coerce_requires_clone(db);
                    struct_.check_f32_precision_loss(db);
                }
                ShimClass::Discriminated(class) => {
                    let enum_ = class.src(db);
                    enum_.check_coerce_name_collisions(db);
                    enum_.check_coerce_requires_clone(db);
                    enum_.check_f32_precision_loss(db);
                    enum_.check_js_name_collisions(db);
                }
            }

            for method in class.methods(db) {
                method.src(db).check_f32_precision_loss(db);
                method.src(db).check_mutable_reference_ignored(db);
            }
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

        assert_eq!(diagnostic_codes(&db), ["rustier::f32_precision_loss"]);
    }
}
