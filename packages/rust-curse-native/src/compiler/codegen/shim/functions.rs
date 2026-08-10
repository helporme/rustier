use crate::{
    compiler::{
        codegen::{
            api::Api,
            fragment::{FragmentOnce, FragmentPrint as _, Printer, comma_list, fragment},
            shim::{
                cast_graph::{Cast, ShimCast, ShimCastCompiler},
                tethering::{
                    TetheringStructType, render_tethering_struct_get_field_expr,
                    render_tethering_struct_set_field_stmt,
                },
            },
        },
        db::Db,
        ir::{
            rust::{AsItem as _, Type},
            shim::{
                ShimFunction, ShimFunctionFree, ShimFunctionGetter, ShimFunctionKind,
                ShimFunctionSetter, ShimType,
            },
        },
    },
    frag,
};

#[salsa::tracked]
impl<'db> ShimFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db) -> String {
        self.render_with(db, ShimCast::Graph).print()
    }

    pub(super) fn render_with<CastCompiler>(
        self,
        db: &'db dyn Db,
        cast_compiler: CastCompiler,
    ) -> impl FragmentOnce
    where
        CastCompiler: ShimCastCompiler<'db> + Copy + Clone + 'db,
    {
        FunctionFragment {
            function: self,
            cast_compiler,
            db,
        }
    }
}

#[derive(Copy, Clone)]
struct FunctionFragment<'db, CastCompiler> {
    function: ShimFunction<'db>,
    cast_compiler: CastCompiler,
    db: &'db dyn Db,
}

impl<'db, CastCompiler> FragmentOnce for FunctionFragment<'db, CastCompiler>
where
    CastCompiler: ShimCastCompiler<'db> + Copy + Clone + 'db,
{
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db, CastCompiler> FunctionFragment<'db, CastCompiler>
where
    CastCompiler: ShimCastCompiler<'db> + Copy + Clone + 'db,
{
    fn render(self) -> impl FragmentOnce {
        let api = Api::get(self.db);

        let ret_type = self.function.ret_type(self.db);

        let result_type = fragment(move |p| match ret_type {
            Some(type_) => api
                .result_type(self.db, type_.render_type_decl(self.db))
                .apply(p),

            None => api.result_type(self.db, "()").apply(p),
        });

        let symbol = self.function.symbol(self.db);

        frag!(
            {api.header_renamed(self.db, symbol)}@
            pub fn ~{symbol}({self.render_params()})~ -> ~{result_type}~ {{
                {self.render_body()}
            }}
        )
    }

    fn render_params(self) -> impl FragmentOnce {
        comma_list(
            self.function
                .params(self.db)
                .iter()
                .enumerate()
                .map(move |(i, param)| {
                    fragment(move |p| {
                        frag!(
                            _{i}: ~{param.type_(self.db).render_type_decl(self.db)}
                        )
                        .apply(p);
                    })
                }),
        )
    }

    fn render_body(self) -> impl FragmentOnce {
        fragment(move |p| match self.function.kind(self.db) {
            ShimFunctionKind::Free(free_fn) => self.render_free_fn_call_body(free_fn).apply(p),
            ShimFunctionKind::Getter(getter) => self.render_fn_getter_body(getter).apply(p),
            ShimFunctionKind::Setter(setter) => self.render_fn_setter_body(setter).apply(p),
        })
    }

    fn render_free_fn_call_body(self, free_fn: ShimFunctionFree<'db>) -> impl FragmentOnce {
        let cast_param_to_source_types =
            self.function
                .params(self.db)
                .iter()
                .enumerate()
                .map(|(i, param)| Cast {
                    from: param.type_(self.db),
                    to: param.src_type(self.db),
                    in_expr: frag!(_{i}),
                    out_var_name: frag!(_{i}),
                });

        let src_fn_call_expr = {
            let params = comma_list(
                free_fn
                    .params(self.db)
                    .iter()
                    .enumerate()
                    .map(|(i, _)| frag!(_{i})),
            );
            frag!({ free_fn.src(self.db).fqn(self.db) }({ params }))
        };

        fragment(move |p| {
            match (
                free_fn.src(self.db).ret_type(self.db),
                free_fn.ret_type(self.db),
            ) {
                (Some(src_ret_type), Some(shim_ret_type)) => {
                    let result_expr = self.cast_compiler.compile_casts_expr(
                        self.db,
                        cast_param_to_source_types,
                        src_fn_call_expr,
                    );

                    let casted_result_expr = self.cast_compiler.compile_cast_expr(
                        self.db,
                        Type::from(src_ret_type),
                        ShimType::from(shim_ret_type),
                        result_expr,
                    );

                    frag!(Ok({ casted_result_expr })).apply(p);
                }
                _ => {
                    let fn_call_stmt = self.cast_compiler.compile_casts_stmt(
                        self.db,
                        cast_param_to_source_types,
                        frag!({src_fn_call_expr};),
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

    fn render_fn_getter_body(self, getter: ShimFunctionGetter<'db>) -> impl FragmentOnce {
        let cls_tethering_struct_expr = self.cast_compiler.compile_cast_expr(
            self.db,
            ShimType::from(getter.class(self.db).as_value_type(self.db)),
            TetheringStructType {
                source_type: getter.class(self.db).src(self.db).into(),
            },
            frag!(_0),
        );

        let field_tethering_value_expr = render_tethering_struct_get_field_expr(
            cls_tethering_struct_expr,
            getter.src(self.db).mangled_name(self.db),
        );

        let ret_type = getter.ret_type(self.db);

        let output = fragment(move |p| {
            if ret_type.uses_tethering() {
                self.cast_compiler
                    .compile_cast_expr(
                        self.db,
                        TetheringStructType {
                            source_type: getter.src(self.db).type_(self.db),
                        },
                        ShimType::from(ret_type),
                        field_tethering_value_expr,
                    )
                    .apply(p);
            } else {
                self.cast_compiler
                    .compile_cast_expr(
                        self.db,
                        Type::from(getter.src(self.db).type_(self.db)),
                        ShimType::from(ret_type),
                        field_tethering_value_expr,
                    )
                    .apply(p);
            }
        });

        frag!(Ok({ output }))
    }

    fn render_fn_setter_body(self, setter: ShimFunctionSetter<'db>) -> impl FragmentOnce {
        let cls_tethering_struct_expr = self.cast_compiler.compile_cast_expr(
            self.db,
            ShimType::from(setter.class(self.db).as_value_type(self.db)),
            TetheringStructType {
                source_type: setter.class(self.db).src(self.db).into(),
            },
            frag!(_0),
        );

        let set_tethered_field_stmt = render_tethering_struct_set_field_stmt(
            cls_tethering_struct_expr,
            setter.src(self.db).mangled_name(self.db),
            frag!(value),
        );

        let set_tethered_field_stmt = self.cast_compiler.compile_cast_stmt(
            self.db,
            Cast {
                from: setter.set_type(self.db),
                to: Type::Value(setter.src(self.db).type_(self.db)),
                in_expr: frag!(_1),
                out_var_name: frag!(value),
            },
            set_tethered_field_stmt,
        );

        frag!(
            {set_tethered_field_stmt}
            Ok()
        )
    }
}
