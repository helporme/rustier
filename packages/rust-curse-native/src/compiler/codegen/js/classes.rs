use crate::{
    compiler::{
        codegen::{
            cast::DirectCastRule,
            fragment::{FragmentOnce, FragmentPrint as _, Printer, fragment},
            js::{
                cast_graph::{Cast, FacadeCast, FacadeCastCompiler},
                types::FacadeFfiType,
            },
        },
        db::Db,
        ir::facade::{
            FacadeClass, FacadeField, FacadeFieldDiscriminated, FacadeFieldDiscriminator,
            FacadeFieldPlain, FacadeType,
        },
    },
    frag,
};

#[salsa::tracked]
impl<'db> FacadeClass<'db> {
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
        FacadeClassRenderer {
            class: self,
            db,
            cast_compiler,
        }
    }
}

#[derive(Copy, Clone)]
struct FacadeClassRenderer<'db, CastCompiler> {
    class: FacadeClass<'db>,
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
}

impl<'db, CastCompiler> FragmentOnce for FacadeClassRenderer<'db, CastCompiler>
where
    CastCompiler: FacadeCastCompiler<'db> + Copy + Clone + 'db,
{
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db, CastCompiler> FacadeClassRenderer<'db, CastCompiler>
where
    CastCompiler: FacadeCastCompiler<'db> + Copy + Clone + 'db,
{
    fn render(self) -> impl FragmentOnce {
        let name = self.class.name(self.db);

        frag!(
            export class~{name}~{{
                constructor(token) {{
                    if (token !== ~{name}.#_construction_token) {{
                        throw new TypeError("rust-backed structs cannot be constructed directly (yet)");
                    }}
                }}@@

                static $from_ffi(ffi) {{
                    const obj = new ~{name}(~{name}.#_construction_token);@
                    obj.#_ffi = ffi;@
                    return obj;
                }}@@

                $ffi() {{
                    return this.#_ffi;
                }}
            }}
        )
    }
}

struct FacadeFieldRenderer<'db, CastCompiler> {
    field: FacadeField<'db>,
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
}

impl<'db, CastCompiler> FacadeFieldRenderer<'db, CastCompiler>
where
    CastCompiler: FacadeCastCompiler<'db> + Copy + Clone + 'db,
{
    fn render(self) -> impl FragmentOnce + 'db {
        fragment(|p| match self.field {
            FacadeField::Plain(plain) => self.render_plain(plain).apply(p),
            FacadeField::Discriminated(discriminated) => {
                self.render_discriminated(discriminated).apply(p)
            }
            FacadeField::Discriminator(discriminator) => {
                self.render_discriminator(discriminator).apply(p)
            }
        })
    }

    fn render_plain(self, field: FacadeFieldPlain<'db>) -> impl FragmentOnce + 'db {
        fragment(move |p| {
            let name = field.name(self.db);

            let ffi_getter = field.ffi_getter(self.db);
            let ffi_value_expr = frag!({ ffi_getter.symbol(self.db) }(this.#_ffi));

            let set_cache_value_stmt = self.cast_compiler.compile_cast_stmt(
                self.db,
                Cast {
                    from: FacadeFfiType {
                        inner: field.type_(self.db),
                    },
                    to: field.type_(self.db),
                    in_expr: ffi_value_expr,
                    out_var_name: "out",
                },
                frag!(this.#__{name} = out),
            );

            frag!(
                #__{name}@
                get ~{name}() ~{{
                    if~(this.#__{name}~=== undefined)~{{
                        {set_cache_value_stmt}
                    }}@@

                    return this.#__{name}
                }}
            )
            .apply(p);

            let Some(ffi_setter) = field.ffi_setter(self.db) else {
                return;
            };

            let ffi_set_stmt = self.cast_compiler.compile_cast_stmt(
                self.db,
                Cast {
                    from: field.type_(self.db),
                    to: FacadeFfiType {
                        inner: field.type_(self.db),
                    },
                    in_expr: value,
                    out_var_name: "out",
                },
                frag!({ ffi_setter.symbol(self.db) }(this.#_ffi, out)),
            );
        })
    }

    fn render_discriminated(self, field: FacadeFieldDiscriminated<'db>) -> impl FragmentOnce + 'db {
        ()
    }

    fn render_discriminator(self, field: FacadeFieldDiscriminator<'db>) -> impl FragmentOnce + 'db {
        ()
    }
}

pub struct CastFacadeClassTypeToFfiClassTypeRule;

impl<'db> DirectCastRule<'db, FacadeType<'db>, FacadeFfiType<'db>>
    for CastFacadeClassTypeToFfiClassTypeRule
{
    fn predecessor(self, _db: &'db dyn Db, to: FacadeFfiType<'db>) -> Option<FacadeType<'db>> {
        match &to.inner.kind {
            FacadeType::Class(..) => Some(to.inner),
            _ => None,
        }
    }

    fn render<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        from: FacadeType<'db>,
        _to: FacadeFfiType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let FacadeType::Class(from) = from.kind else {
            unreachable!()
        };

        frag!(
            {in_expr}.$ffi()
        )
    }
}

pub struct CastFfiClassTypeToFacadeClassTypeRule;

impl<'db> DirectCastRule<'db, FacadeFfiType<'db>, FacadeType<'db>>
    for CastFfiClassTypeToFacadeClassTypeRule
{
    fn predecessor(self, _db: &'db dyn Db, to: FacadeType<'db>) -> Option<FacadeFfiType<'db>> {
        match to.kind {
            FacadeType::Class(..) => Some(FacadeFfiType { inner: to }),
            _ => None,
        }
    }

    fn render<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: FacadeFfiType<'db>,
        to: FacadeType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let FacadeType::Class(to) = to.kind else {
            unreachable!()
        };

        frag!(
            {to.name(db)}.$from_ffi({in_expr})
        )
    }
}
