//! Module for compiling standalone (not associated) functions into a binding
//! functions that can be exposed to JavaScript.

use crate::codegen::{
    bindings::{
        cast_graph::{BindingsCast, BindingsCastCompiler},
        extern_fn::{ExternFn, ExternFnParam, compile_extern_fn},
    },
    db::Db,
    fragment::{FragmentOnce, FragmentPrint, comma_list, frag, fragment},
    ir::{Fn, FnParam, ItemIdExt as _},
};

impl<'db> Fn<'db> {
    pub fn into_extern_fn(
        self,
        db: &'db dyn Db,
    ) -> ExternFn<
        'db,
        impl Iterator<Item = ExternFnParam<'db>> + Clone + 'db,
        impl FragmentOnce + 'db,
    > {
        let params = self.params(db).iter().map(|p| p.into_extern_param(db));

        ExternFn {
            name: self.extern_name(db),
            params,
            return_type: self.return_type(db),
            source_expr: compile_src_fn_call_expr(db, self),
        }
    }
}

#[salsa::tracked]
impl<'db> FnParam<'db> {
    pub fn into_extern_param(self, db: &'db dyn Db) -> ExternFnParam<'db> {
        ExternFnParam {
            name: self.name(db),
            type_: self.type_(db),
            is_self: false,
        }
    }
}

/// Compiles a function into a binding function.
#[salsa::tracked(returns(ref))]
pub fn compile_free_fn<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> String {
    compile_free_fn_with(db, BindingsCast::Graph, fn_)
}

fn compile_free_fn_with<'db, CastCompiler>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    fn_: Fn<'db>,
) -> String
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
{
    let extern_fn = fn_.into_extern_fn(db);
    compile_extern_fn(db, cast_compiler, extern_fn).print()
}

/// Compiles a function into a call statement of the function.
///
/// Assumes the binding parameter variables present in the scope.
fn compile_src_fn_call_expr<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> impl FragmentOnce + 'db {
    let params = fragment(move |printer| {
        if fn_.params(db).is_empty() {
            "()".apply(printer);
        } else {
            frag!(({ comma_list(fn_.params(db).iter().map(|param| param.name(db))) }))
                .apply(printer);
        }
    });

    frag!({fn_.fqn(db)}{params})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assert_rust_snap,
        codegen::{
            bindings::cast_graph::test::TestCastCompiler, lower::rustdoc_to_ir::fns::lower_fn,
        },
    };

    macro_rules! test_fn {
        ($name:ident, $($source:tt)*) => {
            #[test]
            fn $name() {
                assert_rust_snap! {
                    source: { $($source)* },
                    expression: |db, p| {
                        let fn_ = lower_fn(db, db.rustdoc_id("test_fn")).unwrap();
                        compile_free_fn_with(db, TestCastCompiler, fn_).apply(p);
                    },
                }
            }
        };
    }

    test_fn!(zero_args_without_return, pub fn test_fn() {});

    test_fn!(
        zero_args_with_return,
        pub fn test_fn() -> i32 {
            0
        }
    );

    test_fn!(one_arg_without_return, pub fn test_fn(_value: i32) {});

    test_fn!(
        one_arg_with_return,
        pub fn test_fn(value: i32) -> i32 {
            value
        }
    );

    test_fn!(
        two_args_without_return,
        pub fn test_fn(_direct: i32, _non_direct: &i32) {}
    );

    test_fn!(
        two_args_with_return,
        pub fn test_fn(direct: i32, non_direct: &i32) -> i32 {
            direct + non_direct
        }
    );
}
