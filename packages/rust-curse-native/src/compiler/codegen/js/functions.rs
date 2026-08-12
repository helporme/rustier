use crate::{
    compiler::{
        codegen::{
            fragment::{FragmentOnce, FragmentPrint, Printer, comma_list, fragment},
            js::{
                cast_graph::{Cast, FacadeCast, FacadeCastCompiler},
                types::FacadeFfiType,
            },
        },
        db::Db,
        ir::facade::FacadeFunction,
    },
    frag,
};

#[salsa::tracked]
impl<'db> FacadeFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db) -> String {
        self.render_with(db, FacadeCast::Graph).print()
    }

    fn render_with<CastCompiler>(
        self,
        db: &'db dyn Db,
        cast_compiler: CastCompiler,
    ) -> impl FragmentOnce
    where
        CastCompiler: FacadeCastCompiler<'db> + Copy + Clone + 'db,
    {
        FacadeFunctionRenderer {
            function: self,
            db,
            cast_compiler,
        }
    }
}

#[derive(Copy, Clone)]
struct FacadeFunctionRenderer<'db, CastCompiler> {
    function: FacadeFunction<'db>,
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
}

impl<'db, CastCompiler> FragmentOnce for FacadeFunctionRenderer<'db, CastCompiler>
where
    CastCompiler: FacadeCastCompiler<'db> + Copy + Clone + 'db,
{
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db, CastCompiler> FacadeFunctionRenderer<'db, CastCompiler>
where
    CastCompiler: FacadeCastCompiler<'db> + Copy + Clone + 'db,
{
    fn render(self) -> impl FragmentOnce {
        let name = self.function.name(self.db);

        let body = fragment(move |p| match self.function.ret_type(self.db) {
            Some(ret_type) => {
                self.render_param_casts({
                    self.cast_compiler.compile_cast_stmt(
                        self.db,
                        Cast {
                            from: FacadeFfiType { inner: ret_type },
                            to: ret_type,
                            in_expr: self.render_call_expr(),
                            out_var_name: "result",
                        },
                        frag!(return result;),
                    )
                })
                .apply(p);
            }
            None => {
                let call_stmt = frag!({ self.render_call_expr()};);
                self.render_param_casts(call_stmt).apply(p);
            }
        });

        frag!(
            export function~{name}~({self.render_params()})~{{
                {body}@
            }}
        )
    }

    fn render_params(self) -> impl FragmentOnce {
        comma_list(
            self.function
                .params(self.db)
                .iter()
                .enumerate()
                .map(|(i, _)| Self::param_name(i)),
        )
    }

    fn render_param_casts(self, continuation: impl FragmentOnce + 'db) -> impl FragmentOnce + 'db {
        let param_casts = self
            .function
            .params(self.db)
            .iter()
            .enumerate()
            .map(|(i, param)| {
                let type_ = *param.type_(self.db);
                Cast {
                    from: type_,
                    to: FacadeFfiType { inner: type_ },
                    in_expr: Self::param_name(i),
                    out_var_name: Self::casted_param_name(i),
                }
            });

        self.cast_compiler
            .compile_casts_stmt(self.db, param_casts, continuation)
    }

    fn render_call_expr(self) -> impl FragmentOnce {
        let ffi_fn_symbol = self.function.ffi(self.db).symbol(self.db);
        let params = comma_list(
            self.function
                .params(self.db)
                .iter()
                .enumerate()
                .map(|(i, _)| Self::casted_param_name(i)),
        );

        frag!({ ffi_fn_symbol }({ params }));
    }

    fn param_name(i: usize) -> impl FragmentOnce {
        frag!(_{i})
    }

    fn casted_param_name(i: usize) -> impl FragmentOnce {
        frag!(__ { i })
    }
}
