//! Module for compiling standalone (not associated) functions into a binding
//! functions that can be exposed to JavaScript.

use crate::codegen::{
    api::Api,
    bindings::{
        bir::BirSourceType,
        cast_graph::{Cast, compile_cast_expr, compile_casts_expr, compile_casts_stmt},
    },
    db::Db,
    fragment::{FragmentOnce, FragmentPrint, comma_list, frag, fragment},
    ir::{
        fn_::{Fn, FnParam, PassingMode},
        item::ItemIdExt as _,
    },
};

#[salsa::tracked]
impl<'db> FnParam<'db> {
    /// Returns the binding name of the parameter, e.g. `__p__foo`.
    #[salsa::tracked(returns(ref))]
    pub fn binding_name(self, db: &'db dyn Db) -> String {
        format!("__p__{}", self.name(db))
    }

    /// Source type of the parameter.
    pub fn source_type(self, db: &'db dyn Db) -> BirSourceType<'db> {
        match self.mode(db) {
            PassingMode::Ref => BirSourceType::Ref {
                is_mutable: false,
                inner: Box::new(self.type_(db).source_type_owned()),
            },
            PassingMode::Mut => BirSourceType::Ref {
                is_mutable: true,
                inner: Box::new(self.type_(db).source_type_owned()),
            },
            PassingMode::Move => self.type_(db).source_type_owned(),
        }
    }
}

/// Compiles a function into a binding function.
#[salsa::tracked(returns(ref))]
pub fn compile_binding_fn<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> String {
    let api = Api::get(db);
    let result_type = fragment(|p| match fn_.return_type(db) {
        Some(type_) => api.result_type(db, type_.boundary_out().decl(db)).apply(p),
        None => api.result_type(db, "()").apply(p),
    });

    frag!(
        {api.fn_header(db, fn_)}@
        pub fn ~{fn_.extern_name(db)}({compile_binding_fn_params(db, fn_)})~ -> ~{result_type}~ {{
            {compile_binding_fn_body(db, fn_)}
        }}
    )
    .print()
}

fn compile_binding_fn_params<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> impl FragmentOnce + 'db {
    frag!({
        comma_list(fn_.params(db).iter().map(|param| {
            frag!(
                {param.binding_name(db)}:~
                {param.type_(db).boundary_in().decl(db)}
            )
        }))
    })
}

/// Compiles a binding function body for a function returning the active
/// [`crate::codegen::api::Api::result_type`] type, allowing generated `?`
/// expressions.
fn compile_binding_fn_body<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> impl FragmentOnce + 'db {
    fragment(move |p| {
        let cast_param_to_source_types = fn_.params(db).iter().map(|param| Cast {
            from: param.type_(db).boundary_in(),
            to: param.source_type(db),
            in_expr: param.binding_name(db),
            out_var_name: param.binding_name(db),
        });

        match fn_.return_type(db) {
            Some(ret_type) => {
                let result_expr = compile_casts_expr(
                    db,
                    cast_param_to_source_types,
                    compile_src_fn_call_expr(db, fn_),
                );

                let casted_result_expr = compile_cast_expr(
                    db,
                    ret_type.source_type_owned(),
                    ret_type.boundary_out(),
                    result_expr,
                );

                frag!(Ok({ casted_result_expr })).apply(p);
            }
            None => {
                let fn_call_stmt = compile_casts_stmt(
                    db,
                    cast_param_to_source_types,
                    frag!({compile_src_fn_call_expr(db, fn_)};),
                );

                frag!(
                    {fn_call_stmt}@
                    Ok(())
                )
                .apply(p);
            }
        }
    })
}

/// Compiles a function into a call statement of the function.
///
/// Assumes the binding parameter variables present in the scope.
fn compile_src_fn_call_expr<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> impl FragmentOnce + 'db {
    let params = fragment(move |printer| {
        if fn_.params(db).is_empty() {
            "()".apply(printer);
        } else {
            frag!(({ comma_list(fn_.params(db).iter().map(|param| param.binding_name(db))) }))
                .apply(printer);
        }
    });

    frag!({fn_.fqn(db)}{params})
}
