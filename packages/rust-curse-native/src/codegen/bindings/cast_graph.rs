//! Rust-specific utilities for compiling casts used by generated bindings.

use crate::{
    codegen::{
        bindings::{
            bir::{ExternType, SourceType},
            extern_class::{
                CastClassBoundaryTypeToTetheringStructType,
                CastTetheringStructTypeToClassBoundaryType,
            },
            primitive::{
                CastCharToU32BoundaryRule, CastF32ToF64BoundaryRule, CastF64ToF32Rule,
                CastF64ToSignedIntRule, CastF64ToUnsignedIntRule, CastJsAnyToBoolRule,
                CastJsAnyToF64Rule, CastJsAnyToStringRule, CastPrimitiveToSameBoundaryRule,
                CastStringToStrMutRule, CastStringToStrRefRule, CastU32ToCharRule,
            },
            tethering::{
                CastSourceTypeToTetheringStructType, CastTetheringStructTypeToBorrowedSourceType,
                CastTetheringStructTypeToOwnedSourceType, TetheringStructType,
            },
        },
        cast::{CastContinuation, IndirectCastRule},
        db::Db,
        fragment::{FragmentOnce, Printer, fragment},
    },
    define_cast_graph, frag,
};

define_cast_graph! {
    pub BindingsCast<'db> {
        types {
            BoundaryType(ExternType<'db>),
            SourceType(SourceType<'db>),
            TetheringStructType(TetheringStructType<'db>),
        }

        direct_rules {
            JsAnyToF64 {
                rule: CastJsAnyToF64Rule,
                from: BoundaryType,
                to: SourceType,
            }
            JsAnyToBool {
                rule: CastJsAnyToBoolRule,
                from: BoundaryType,
                to: SourceType,
            }
            JsAnyToString {
                rule: CastJsAnyToStringRule,
                from: BoundaryType,
                to: SourceType,
            }
            F64ToSignedInt {
                rule: CastF64ToSignedIntRule,
                from: SourceType,
                to: SourceType,
            }
            F64ToUnsignedInt {
                rule: CastF64ToUnsignedIntRule,
                from: SourceType,
                to: SourceType,
            }
            U32ToChar {
                rule: CastU32ToCharRule,
                from: SourceType,
                to: SourceType,
            }
            F64ToF32 {
                rule: CastF64ToF32Rule,
                from: SourceType,
                to: SourceType,
            }
            CharToU32Boundary {
                rule: CastCharToU32BoundaryRule,
                from: SourceType,
                to: BoundaryType,
            }
            F32ToF64Boundary {
                rule: CastF32ToF64BoundaryRule,
                from: SourceType,
                to: BoundaryType,
            }
            PrimitiveToSameBoundary {
                rule: CastPrimitiveToSameBoundaryRule,
                from: SourceType,
                to: BoundaryType,
            }
            SourceTypeToTetheringStructType {
                rule: CastSourceTypeToTetheringStructType,
                from: SourceType,
                to: TetheringStructType,
            }
            ClassBoundaryTypeToTetheringStructType {
                rule: CastClassBoundaryTypeToTetheringStructType,
                from: BoundaryType,
                to: TetheringStructType
            }
            TetheringStructTypeToClassBoundaryType {
                rule: CastTetheringStructTypeToClassBoundaryType,
                from: TetheringStructType,
                to: BoundaryType
            }
        }

        indirect_rules {
            OwnedTypeToBorrowedType {
                rule: CastOwnedTypeToBorrowedTypeRule,
                from: SourceType,
                to: SourceType,
            }
            StringToStrRef {
                rule: CastStringToStrRefRule,
                from: SourceType,
                to: SourceType,
            }
            StringToStrMut {
                rule: CastStringToStrMutRule,
                from: SourceType,
                to: SourceType,
            }
            TetheringStructTypeToBorrowedSourceType {
                rule: CastTetheringStructTypeToBorrowedSourceType,
                from: TetheringStructType,
                to: SourceType,
            }
            TetheringStructTypeToOwnedSourceType {
                rule: CastTetheringStructTypeToOwnedSourceType,
                from: TetheringStructType,
                to: SourceType,
            }
        }
    }
}

/// A Rust cast request whose converted value is bound to `out_var_name`.
pub struct Cast<'db, From, To, Expr> {
    pub from: From,
    pub to: To,
    pub in_expr: Expr,
    pub out_var_name: &'db str,
}

/// Compiles cast fragments for generated Rust bindings.
pub trait BindingsCastCompiler<'db> {
    /// Compiles one cast into a named Rust binding before `inner_stmt`.
    fn compile_cast_stmt<From, To, Expr, Continue>(
        self,
        db: &'db dyn Db,
        cast: Cast<'db, From, To, Expr>,
        inner_stmt: Continue,
    ) -> impl FragmentOnce + 'db
    where
        From: Into<BindingsCast::CastType<'db>> + 'db,
        To: Into<BindingsCast::CastType<'db>> + 'db,
        Expr: FragmentOnce + 'db,
        Continue: FragmentOnce + 'db;

    /// Compiles multiple named casts in order before `inner_stmt`.
    fn compile_casts_stmt<Casts, From, To, Expr, Continue>(
        self,
        db: &'db dyn Db,
        casts: Casts,
        inner_stmt: Continue,
    ) -> impl FragmentOnce + 'db
    where
        Casts: IntoIterator<Item = Cast<'db, From, To, Expr>> + 'db,
        Casts::IntoIter: 'db,
        From: Into<BindingsCast::CastType<'db>> + 'db,
        To: Into<BindingsCast::CastType<'db>> + 'db,
        Expr: FragmentOnce + 'db,
        Continue: FragmentOnce + 'db;

    /// Compiles one named cast around a Rust expression and captures its
    /// result.
    fn compile_cast_expr<From, To, Expr>(
        self,
        db: &'db dyn Db,
        from: From,
        to: To,
        expr: Expr,
    ) -> impl FragmentOnce + 'db
    where
        From: Into<BindingsCast::CastType<'db>> + 'db,
        To: Into<BindingsCast::CastType<'db>> + 'db,
        Expr: FragmentOnce + 'db;

    /// Compiles named casts around a Rust expression and captures its result
    /// only when an indirect cast scopes the expression.
    fn compile_casts_expr<Casts, From, To, Expr, Inner>(
        self,
        db: &'db dyn Db,
        casts: Casts,
        inner_expr: Inner,
    ) -> impl FragmentOnce + 'db
    where
        Casts: IntoIterator<Item = Cast<'db, From, To, Expr>> + 'db,
        Casts::IntoIter: 'db,
        From: Into<BindingsCast::CastType<'db>> + 'db,
        To: Into<BindingsCast::CastType<'db>> + 'db,
        Expr: FragmentOnce + 'db,
        Inner: FragmentOnce + 'db;
}

/// Compiles one cast into a named Rust binding before `inner_stmt`.
pub fn compile_cast_stmt<'db, From, To, Expr, Continue>(
    db: &'db dyn Db,
    cast: Cast<'db, From, To, Expr>,
    inner_stmt: Continue,
) -> impl FragmentOnce + 'db
where
    From: Into<BindingsCast::CastType<'db>> + 'db,
    To: Into<BindingsCast::CastType<'db>> + 'db,
    Expr: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    BindingsCast::compile_cast(
        db,
        cast.from,
        cast.to,
        cast.in_expr,
        BindCastOutput {
            out_var_name: cast.out_var_name,
            continue_with: inner_stmt,
        },
    )
}

/// Compiles multiple named casts in order before `inner_stmt`.
pub fn compile_casts_stmt<'db, Casts, From, To, Expr, Continue>(
    db: &'db dyn Db,
    casts: Casts,
    inner_stmt: Continue,
) -> impl FragmentOnce + 'db
where
    Casts: IntoIterator<Item = Cast<'db, From, To, Expr>> + 'db,
    Casts::IntoIter: 'db,
    From: Into<BindingsCast::CastType<'db>> + 'db,
    To: Into<BindingsCast::CastType<'db>> + 'db,
    Expr: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    CompileCastsStmt {
        db,
        casts: casts.into_iter(),
        inner_stmt,
    }
}

/// Sequences Rust binding casts while keeping each named output in scope.
struct CompileCastsStmt<'db, Casts, Continue> {
    db: &'db dyn Db,
    casts: Casts,
    inner_stmt: Continue,
}

impl<'db, Casts, From, To, Expr, Continue> FragmentOnce for CompileCastsStmt<'db, Casts, Continue>
where
    Casts: Iterator<Item = Cast<'db, From, To, Expr>> + 'db,
    From: Into<BindingsCast::CastType<'db>> + 'db,
    To: Into<BindingsCast::CastType<'db>> + 'db,
    Expr: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    fn apply(mut self, printer: &mut Printer) {
        let Some(cast) = self.casts.next() else {
            self.inner_stmt.apply(printer);
            return;
        };

        let remaining = Self {
            db: self.db,
            casts: self.casts,
            inner_stmt: self.inner_stmt,
        };
        compile_cast_stmt(self.db, cast, remaining).apply(printer);
    }
}

/// Compiles one named cast around a Rust expression and captures its result.
pub fn compile_cast_expr<'db, From, To, Expr>(
    db: &'db dyn Db,
    from: From,
    to: To,
    expr: Expr,
) -> impl FragmentOnce + 'db
where
    From: Into<BindingsCast::CastType<'db>> + 'db,
    To: Into<BindingsCast::CastType<'db>> + 'db,
    Expr: FragmentOnce + 'db,
{
    let from = from.into();
    let to = to.into();
    let uses_indirect_rule = BindingsCast::find_cast_path(db, from.clone(), to.clone())
        .iter()
        .any(|step| step.rule.is_indirect());

    fragment(move |printer| {
        if uses_indirect_rule {
            frag!({{
                let mut __cast_expr_result = None;@
                {BindingsCast::compile_cast(
                    db,
                    from,
                    to,
                    expr,
                    WriteCastOutput {
                        out_var_name: "__cast_expr_result",
                    },
                )}@
                __cast_expr_result.unwrap()
            }})
            .apply(printer);
        } else {
            BindingsCast::compile_cast(db, from, to, expr, ReturnCastOutput).apply(printer);
        }
    })
}

/// Compiles named casts around a Rust expression and captures its result only
/// when an indirect cast scopes the expression.
pub fn compile_casts_expr<'db, Casts, From, To, Expr, Inner>(
    db: &'db dyn Db,
    casts: Casts,
    inner_expr: Inner,
) -> impl FragmentOnce + 'db
where
    Casts: IntoIterator<Item = Cast<'db, From, To, Expr>> + 'db,
    Casts::IntoIter: 'db,
    From: Into<BindingsCast::CastType<'db>> + 'db,
    To: Into<BindingsCast::CastType<'db>> + 'db,
    Expr: FragmentOnce + 'db,
    Inner: FragmentOnce + 'db,
{
    CompileCastsExpr {
        db,
        casts: casts.into_iter(),
        inner_expr,
    }
}

impl<'db> BindingsCastCompiler<'db> for BindingsCast::Graph {
    fn compile_cast_stmt<From, To, Expr, Continue>(
        self,
        db: &'db dyn Db,
        cast: Cast<'db, From, To, Expr>,
        inner_stmt: Continue,
    ) -> impl FragmentOnce + 'db
    where
        From: Into<BindingsCast::CastType<'db>> + 'db,
        To: Into<BindingsCast::CastType<'db>> + 'db,
        Expr: FragmentOnce + 'db,
        Continue: FragmentOnce + 'db,
    {
        compile_cast_stmt(db, cast, inner_stmt)
    }

    fn compile_casts_stmt<Casts, From, To, Expr, Continue>(
        self,
        db: &'db dyn Db,
        casts: Casts,
        inner_stmt: Continue,
    ) -> impl FragmentOnce + 'db
    where
        Casts: IntoIterator<Item = Cast<'db, From, To, Expr>> + 'db,
        Casts::IntoIter: 'db,
        From: Into<BindingsCast::CastType<'db>> + 'db,
        To: Into<BindingsCast::CastType<'db>> + 'db,
        Expr: FragmentOnce + 'db,
        Continue: FragmentOnce + 'db,
    {
        compile_casts_stmt(db, casts, inner_stmt)
    }

    fn compile_cast_expr<From, To, Expr>(
        self,
        db: &'db dyn Db,
        from: From,
        to: To,
        expr: Expr,
    ) -> impl FragmentOnce + 'db
    where
        From: Into<BindingsCast::CastType<'db>> + 'db,
        To: Into<BindingsCast::CastType<'db>> + 'db,
        Expr: FragmentOnce + 'db,
    {
        compile_cast_expr(db, from, to, expr)
    }

    fn compile_casts_expr<Casts, From, To, Expr, Inner>(
        self,
        db: &'db dyn Db,
        casts: Casts,
        inner_expr: Inner,
    ) -> impl FragmentOnce + 'db
    where
        Casts: IntoIterator<Item = Cast<'db, From, To, Expr>> + 'db,
        Casts::IntoIter: 'db,
        From: Into<BindingsCast::CastType<'db>> + 'db,
        To: Into<BindingsCast::CastType<'db>> + 'db,
        Expr: FragmentOnce + 'db,
        Inner: FragmentOnce + 'db,
    {
        compile_casts_expr(db, casts, inner_expr)
    }
}

/// Sequences expression casts and captures only continuations scoped by an
/// indirect cast.
struct CompileCastsExpr<'db, Casts, Inner> {
    db: &'db dyn Db,
    casts: Casts,
    inner_expr: Inner,
}

impl<'db, Casts, From, To, Expr, Inner> FragmentOnce for CompileCastsExpr<'db, Casts, Inner>
where
    Casts: Iterator<Item = Cast<'db, From, To, Expr>> + 'db,
    From: Into<BindingsCast::CastType<'db>> + 'db,
    To: Into<BindingsCast::CastType<'db>> + 'db,
    Expr: FragmentOnce + 'db,
    Inner: FragmentOnce + 'db,
{
    fn apply(mut self, printer: &mut Printer) {
        let Some(cast) = self.casts.next() else {
            self.inner_expr.apply(printer);
            return;
        };

        let Cast {
            from,
            to,
            in_expr,
            out_var_name,
        } = cast;
        let from = from.into();
        let to = to.into();

        let uses_indirect_rule = BindingsCast::find_cast_path(self.db, from.clone(), to.clone())
            .iter()
            .any(|step| step.rule.is_indirect());

        let remaining = Self {
            db: self.db,
            casts: self.casts,
            inner_expr: self.inner_expr,
        };
        let cast = Cast {
            from,
            to,
            in_expr,
            out_var_name,
        };

        if uses_indirect_rule {
            frag!({{
                let mut __cast_expr_result = None;@
                {compile_cast_stmt(
                    self.db,
                    cast,
                    frag!(__cast_expr_result = Some({remaining});),
                )}@
                __cast_expr_result.unwrap()
            }})
            .apply(printer);
        } else {
            frag!({ { { compile_cast_stmt(self.db, cast, remaining) } } }).apply(printer);
        }
    }
}

/// Returns a direct cast's output as the compiled expression.
struct ReturnCastOutput;

impl<'db> CastContinuation<'db> for ReturnCastOutput {
    fn compile<'fragment, Expr>(self, out_expr: Expr) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        out_expr
    }
}

/// Writes a scoped cast's output into an expression result slot.
struct WriteCastOutput<'db> {
    out_var_name: &'db str,
}

impl<'db> CastContinuation<'db> for WriteCastOutput<'db> {
    fn compile<'fragment, Expr>(self, out_expr: Expr) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let out_var_name = self.out_var_name;
        frag!({out_var_name} = Some({out_expr});)
    }
}

/// Binds a cast result before compiling the remaining Rust statements.
struct BindCastOutput<'db, Continue> {
    out_var_name: &'db str,
    continue_with: Continue,
}

impl<'db, Continue> CastContinuation<'db> for BindCastOutput<'db, Continue>
where
    Continue: FragmentOnce + 'db,
{
    fn compile<'fragment, Expr>(self, out_expr: Expr) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let Self {
            out_var_name,
            continue_with,
        } = self;

        frag!(
            let ~{out_var_name}~ = ~{out_expr};@
            {continue_with}
        )
    }
}

/// A rule to cast an owned source type to a reference source type.
pub struct CastOwnedTypeToBorrowedTypeRule;

impl<'db> IndirectCastRule<'db, SourceType<'db>, SourceType<'db>>
    for CastOwnedTypeToBorrowedTypeRule
{
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<SourceType<'db>> {
        match to {
            SourceType::Ref { inner, .. } if inner.allows_generic_borrowing_cast() => Some(*inner),
            _ => None,
        }
    }

    fn compile<'fragment, Expr, Continue>(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
        to: SourceType<'db>,
        in_expr: Expr,
        continue_with: Continue,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
        Continue: CastContinuation<'db>,
    {
        let SourceType::Ref { is_mutable, .. } = to else {
            unreachable!()
        };

        let borrowing = if is_mutable {
            frag!(&mut __cast_owned_value)
        } else {
            frag!(&__cast_owned_value)
        };
        let continue_with = continue_with.compile(borrowing);

        frag!(
            let mut __cast_owned_value = {in_expr};
            {continue_with}
        )
    }
}

#[cfg(test)]
pub mod test {
    use std::fmt::{Debug, Write as _};

    use crate::{
        codegen::{
            bindings::cast_graph::BindingsCastCompiler,
            fragment::{FragmentOnce, Printer, fragment},
        },
        frag,
    };

    /// Compiles visible fake cast expressions for testing binding code
    /// generation.
    #[derive(Copy, Clone)]
    pub struct TestCastCompiler;

    struct DebugString<T>(T);

    impl<T> FragmentOnce for DebugString<T>
    where
        T: Debug,
    {
        fn apply(self, printer: &mut Printer) {
            let debug = format!("{:?}", self.0);
            write!(printer, "{debug:?}").expect("writing to source printer cannot fail");
        }
    }

    fn compile_test_cast_expr<'db, From, To, Expr>(
        from: From,
        to: To,
        expr: Expr,
    ) -> impl FragmentOnce + 'db
    where
        From: Into<super::BindingsCast::CastType<'db>> + 'db,
        To: Into<super::BindingsCast::CastType<'db>> + 'db,
        Expr: FragmentOnce + 'db,
    {
        let from = DebugString(from.into());
        let to = DebugString(to.into());

        frag!(cast.from({ from }).to({ to }).an({ expr }))
    }

    impl<'db> BindingsCastCompiler<'db> for TestCastCompiler {
        fn compile_cast_stmt<From, To, Expr, Continue>(
            self,
            _db: &'db dyn crate::codegen::db::Db,
            cast: super::Cast<'db, From, To, Expr>,
            inner_stmt: Continue,
        ) -> impl FragmentOnce + 'db
        where
            From: Into<super::BindingsCast::CastType<'db>> + 'db,
            To: Into<super::BindingsCast::CastType<'db>> + 'db,
            Expr: FragmentOnce + 'db,
            Continue: FragmentOnce + 'db,
        {
            let cast_expr = compile_test_cast_expr(cast.from, cast.to, cast.in_expr);

            frag!(
                let ~{cast.out_var_name}~ = {cast_expr};@
                {inner_stmt}
            )
        }

        fn compile_casts_stmt<Casts, From, To, Expr, Continue>(
            self,
            _db: &'db dyn crate::codegen::db::Db,
            casts: Casts,
            inner_stmt: Continue,
        ) -> impl FragmentOnce + 'db
        where
            Casts: IntoIterator<Item = super::Cast<'db, From, To, Expr>> + 'db,
            Casts::IntoIter: 'db,
            From: Into<super::BindingsCast::CastType<'db>> + 'db,
            To: Into<super::BindingsCast::CastType<'db>> + 'db,
            Expr: FragmentOnce + 'db,
            Continue: FragmentOnce + 'db,
        {
            fragment(move |printer| {
                for cast in casts {
                    let cast_expr = compile_test_cast_expr(cast.from, cast.to, cast.in_expr);
                    frag!(let ~{cast.out_var_name}~ = {cast_expr};@).apply(printer);
                }

                inner_stmt.apply(printer);
            })
        }

        fn compile_cast_expr<From, To, Expr>(
            self,
            _db: &'db dyn crate::codegen::db::Db,
            from: From,
            to: To,
            expr: Expr,
        ) -> impl FragmentOnce + 'db
        where
            From: Into<super::BindingsCast::CastType<'db>> + 'db,
            To: Into<super::BindingsCast::CastType<'db>> + 'db,
            Expr: FragmentOnce + 'db,
        {
            compile_test_cast_expr(from, to, expr)
        }

        fn compile_casts_expr<Casts, From, To, Expr, Inner>(
            self,
            _db: &'db dyn crate::codegen::db::Db,
            casts: Casts,
            inner_expr: Inner,
        ) -> impl FragmentOnce + 'db
        where
            Casts: IntoIterator<Item = super::Cast<'db, From, To, Expr>> + 'db,
            Casts::IntoIter: 'db,
            From: Into<super::BindingsCast::CastType<'db>> + 'db,
            To: Into<super::BindingsCast::CastType<'db>> + 'db,
            Expr: FragmentOnce + 'db,
            Inner: FragmentOnce + 'db,
        {
            fragment(move |printer| {
                let mut casts = casts.into_iter();
                let Some(first_cast) = casts.next() else {
                    inner_expr.apply(printer);
                    return;
                };
                let cast_exprs = fragment(move |printer| {
                    for cast in std::iter::once(first_cast).chain(casts) {
                        let cast_expr = compile_test_cast_expr(cast.from, cast.to, cast.in_expr);
                        frag!(let ~{cast.out_var_name}~ = {cast_expr};@).apply(printer);
                    }

                    inner_expr.apply(printer);
                });

                frag!({ { { cast_exprs } } }).apply(printer);
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Cast, compile_cast_stmt};
    use crate::{
        assert_rust_snap,
        codegen::{
            bindings::bir::SourceType,
            fragment::{FragmentOnce as _, FragmentPrint as _},
            ir::type_::{IrPrimitive as Primitive, ItemIdExt as _, Type},
        },
        frag,
    };

    macro_rules! cast_snapshot {
        (
            name: $name:ident,
            from: |$from_db:ident| $from:expr,
            to: |$to_db:ident| $to:expr $(,)?
        ) => {
            #[test]
            fn $name() {
                assert_rust_snap! {
                    source: {
                        #[derive(Clone)]
                        pub struct ExampleStruct;

                        #[derive(Clone)]
                        pub enum ExampleEnum {
                            Variant,
                        }
                    },
                    expression: |__db, printer| {
                        let __db = &*__db;
                        let from = {
                            let $from_db = __db;
                            $from
                        };
                        let to = {
                            let $to_db = __db;
                            $to
                        };

                        frag!(
                            fn cast(input: ())~{{
                                {compile_cast_stmt(
                                    __db,
                                    Cast {
                                        from,
                                        to,
                                        in_expr: "input",
                                        out_var_name: "output",
                                    },
                                    frag!(consume(output);),
                                )}
                            }}
                        )
                        .apply(printer);
                    },
                }
            }
        };
    }

    cast_snapshot! {
        name: bool_in,
        from: |_db| Type::Primitive(Primitive::Boolean).extern_in(),
        to: |_db| Type::Primitive(Primitive::Boolean).source_type_owned(),
    }

    cast_snapshot! {
        name: bool_ref_in,
        from: |_db| Type::Primitive(Primitive::Boolean).extern_in(),
        to: |_db| Type::Primitive(Primitive::Boolean).source_type_owned().into_ref(),
    }

    cast_snapshot! {
        name: bool_mut_in,
        from: |_db| Type::Primitive(Primitive::Boolean).extern_in(),
        to: |_db| Type::Primitive(Primitive::Boolean).source_type_owned().into_mut(),
    }

    cast_snapshot! {
        name: bool_out,
        from: |_db| Type::Primitive(Primitive::Boolean).source_type_owned(),
        to: |_db| Type::Primitive(Primitive::Boolean).extern_out(),
    }

    cast_snapshot! {
        name: char_in,
        from: |_db| Type::Primitive(Primitive::Char).extern_in(),
        to: |_db| Type::Primitive(Primitive::Char).source_type_owned(),
    }

    cast_snapshot! {
        name: char_ref_in,
        from: |_db| Type::Primitive(Primitive::Char).extern_in(),
        to: |_db| Type::Primitive(Primitive::Char).source_type_owned().into_ref(),
    }

    cast_snapshot! {
        name: char_mut_in,
        from: |_db| Type::Primitive(Primitive::Char).extern_in(),
        to: |_db| Type::Primitive(Primitive::Char).source_type_owned().into_mut(),
    }

    cast_snapshot! {
        name: char_out,
        from: |_db| Type::Primitive(Primitive::Char).source_type_owned(),
        to: |_db| Type::Primitive(Primitive::Char).extern_out(),
    }

    cast_snapshot! {
        name: i8_in,
        from: |_db| Type::Primitive(Primitive::i8).extern_in(),
        to: |_db| Type::Primitive(Primitive::i8).source_type_owned(),
    }

    cast_snapshot! {
        name: i8_ref_in,
        from: |_db| Type::Primitive(Primitive::i8).extern_in(),
        to: |_db| Type::Primitive(Primitive::i8).source_type_owned().into_ref(),
    }

    cast_snapshot! {
        name: i8_mut_in,
        from: |_db| Type::Primitive(Primitive::i8).extern_in(),
        to: |_db| Type::Primitive(Primitive::i8).source_type_owned().into_mut(),
    }

    cast_snapshot! {
        name: i8_out,
        from: |_db| Type::Primitive(Primitive::i8).source_type_owned(),
        to: |_db| Type::Primitive(Primitive::i8).extern_out(),
    }

    cast_snapshot! {
        name: u8_in,
        from: |_db| Type::Primitive(Primitive::u8).extern_in(),
        to: |_db| Type::Primitive(Primitive::u8).source_type_owned(),
    }

    cast_snapshot! {
        name: u8_ref_in,
        from: |_db| Type::Primitive(Primitive::u8).extern_in(),
        to: |_db| Type::Primitive(Primitive::u8).source_type_owned().into_ref(),
    }

    cast_snapshot! {
        name: u8_mut_in,
        from: |_db| Type::Primitive(Primitive::u8).extern_in(),
        to: |_db| Type::Primitive(Primitive::u8).source_type_owned().into_mut(),
    }

    cast_snapshot! {
        name: u8_out,
        from: |_db| Type::Primitive(Primitive::u8).source_type_owned(),
        to: |_db| Type::Primitive(Primitive::u8).extern_out(),
    }

    cast_snapshot! {
        name: i16_in,
        from: |_db| Type::Primitive(Primitive::i16).extern_in(),
        to: |_db| Type::Primitive(Primitive::i16).source_type_owned(),
    }

    cast_snapshot! {
        name: i16_ref_in,
        from: |_db| Type::Primitive(Primitive::i16).extern_in(),
        to: |_db| Type::Primitive(Primitive::i16).source_type_owned().into_ref(),
    }

    cast_snapshot! {
        name: i16_mut_in,
        from: |_db| Type::Primitive(Primitive::i16).extern_in(),
        to: |_db| Type::Primitive(Primitive::i16).source_type_owned().into_mut(),
    }

    cast_snapshot! {
        name: i16_out,
        from: |_db| Type::Primitive(Primitive::i16).source_type_owned(),
        to: |_db| Type::Primitive(Primitive::i16).extern_out(),
    }

    cast_snapshot! {
        name: u16_in,
        from: |_db| Type::Primitive(Primitive::u16).extern_in(),
        to: |_db| Type::Primitive(Primitive::u16).source_type_owned(),
    }

    cast_snapshot! {
        name: u16_ref_in,
        from: |_db| Type::Primitive(Primitive::u16).extern_in(),
        to: |_db| Type::Primitive(Primitive::u16).source_type_owned().into_ref(),
    }

    cast_snapshot! {
        name: u16_mut_in,
        from: |_db| Type::Primitive(Primitive::u16).extern_in(),
        to: |_db| Type::Primitive(Primitive::u16).source_type_owned().into_mut(),
    }

    cast_snapshot! {
        name: u16_out,
        from: |_db| Type::Primitive(Primitive::u16).source_type_owned(),
        to: |_db| Type::Primitive(Primitive::u16).extern_out(),
    }

    cast_snapshot! {
        name: i32_in,
        from: |_db| Type::Primitive(Primitive::i32).extern_in(),
        to: |_db| Type::Primitive(Primitive::i32).source_type_owned(),
    }

    cast_snapshot! {
        name: i32_ref_in,
        from: |_db| Type::Primitive(Primitive::i32).extern_in(),
        to: |_db| Type::Primitive(Primitive::i32).source_type_owned().into_ref(),
    }

    cast_snapshot! {
        name: i32_mut_in,
        from: |_db| Type::Primitive(Primitive::i32).extern_in(),
        to: |_db| Type::Primitive(Primitive::i32).source_type_owned().into_mut(),
    }

    cast_snapshot! {
        name: i32_out,
        from: |_db| Type::Primitive(Primitive::i32).source_type_owned(),
        to: |_db| Type::Primitive(Primitive::i32).extern_out(),
    }

    cast_snapshot! {
        name: u32_in,
        from: |_db| Type::Primitive(Primitive::u32).extern_in(),
        to: |_db| Type::Primitive(Primitive::u32).source_type_owned(),
    }

    cast_snapshot! {
        name: u32_ref_in,
        from: |_db| Type::Primitive(Primitive::u32).extern_in(),
        to: |_db| Type::Primitive(Primitive::u32).source_type_owned().into_ref(),
    }

    cast_snapshot! {
        name: u32_mut_in,
        from: |_db| Type::Primitive(Primitive::u32).extern_in(),
        to: |_db| Type::Primitive(Primitive::u32).source_type_owned().into_mut(),
    }

    cast_snapshot! {
        name: u32_out,
        from: |_db| Type::Primitive(Primitive::u32).source_type_owned(),
        to: |_db| Type::Primitive(Primitive::u32).extern_out(),
    }

    cast_snapshot! {
        name: f32_in,
        from: |_db| Type::Primitive(Primitive::f32).extern_in(),
        to: |_db| Type::Primitive(Primitive::f32).source_type_owned(),
    }

    cast_snapshot! {
        name: f32_ref_in,
        from: |_db| Type::Primitive(Primitive::f32).extern_in(),
        to: |_db| Type::Primitive(Primitive::f32).source_type_owned().into_ref(),
    }

    cast_snapshot! {
        name: f32_mut_in,
        from: |_db| Type::Primitive(Primitive::f32).extern_in(),
        to: |_db| Type::Primitive(Primitive::f32).source_type_owned().into_mut(),
    }

    cast_snapshot! {
        name: f32_out,
        from: |_db| Type::Primitive(Primitive::f32).source_type_owned(),
        to: |_db| Type::Primitive(Primitive::f32).extern_out(),
    }

    cast_snapshot! {
        name: f64_in,
        from: |_db| Type::Primitive(Primitive::f64).extern_in(),
        to: |_db| Type::Primitive(Primitive::f64).source_type_owned(),
    }

    cast_snapshot! {
        name: f64_ref_in,
        from: |_db| Type::Primitive(Primitive::f64).extern_in(),
        to: |_db| Type::Primitive(Primitive::f64).source_type_owned().into_ref(),
    }

    cast_snapshot! {
        name: f64_mut_in,
        from: |_db| Type::Primitive(Primitive::f64).extern_in(),
        to: |_db| Type::Primitive(Primitive::f64).source_type_owned().into_mut(),
    }

    cast_snapshot! {
        name: f64_out,
        from: |_db| Type::Primitive(Primitive::f64).source_type_owned(),
        to: |_db| Type::Primitive(Primitive::f64).extern_out(),
    }

    cast_snapshot! {
        name: str_in,
        from: |_db| Type::Str.extern_in(),
        to: |_db| SourceType::Str.into_ref(),
    }

    cast_snapshot! {
        name: str_ref_in,
        from: |_db| Type::Str.extern_in(),
        to: |_db| SourceType::Str.into_ref(),
    }

    cast_snapshot! {
        name: str_mut_in,
        from: |_db| Type::Str.extern_in(),
        to: |_db| SourceType::Str.into_mut(),
    }

    cast_snapshot! {
        name: string_in,
        from: |_db| Type::String.extern_in(),
        to: |_db| Type::String.source_type_owned(),
    }

    cast_snapshot! {
        name: string_ref_in,
        from: |_db| Type::String.extern_in(),
        to: |_db| Type::String.source_type_owned().into_ref(),
    }

    cast_snapshot! {
        name: string_mut_in,
        from: |_db| Type::String.extern_in(),
        to: |_db| Type::String.source_type_owned().into_mut(),
    }

    cast_snapshot! {
        name: string_out,
        from: |_db| Type::String.source_type_owned(),
        to: |_db| Type::String.extern_out(),
    }

    cast_snapshot! {
        name: struct_in,
        from: |db| db.item_id("ExampleStruct").into_ir_type(db).extern_in(),
        to: |db| db.item_id("ExampleStruct").into_ir_type(db).source_type_owned(),
    }

    cast_snapshot! {
        name: struct_ref_in,
        from: |db| db.item_id("ExampleStruct").into_ir_type(db).extern_in(),
        to: |db| db.item_id("ExampleStruct").into_ir_type(db).source_type_owned().into_ref(),
    }

    cast_snapshot! {
        name: struct_mut_in,
        from: |db| db.item_id("ExampleStruct").into_ir_type(db).extern_in(),
        to: |db| db.item_id("ExampleStruct").into_ir_type(db).source_type_owned().into_mut(),
    }

    cast_snapshot! {
        name: struct_out,
        from: |db| db.item_id("ExampleStruct").into_ir_type(db).source_type_owned(),
        to: |db| db.item_id("ExampleStruct").into_ir_type(db).extern_out(),
    }

    cast_snapshot! {
        name: enum_in,
        from: |db| db.item_id("ExampleEnum").into_ir_type(db).extern_in(),
        to: |db| db.item_id("ExampleEnum").into_ir_type(db).source_type_owned(),
    }

    cast_snapshot! {
        name: enum_ref_in,
        from: |db| db.item_id("ExampleEnum").into_ir_type(db).extern_in(),
        to: |db| db.item_id("ExampleEnum").into_ir_type(db).source_type_owned().into_ref(),
    }

    cast_snapshot! {
        name: enum_mut_in,
        from: |db| db.item_id("ExampleEnum").into_ir_type(db).extern_in(),
        to: |db| db.item_id("ExampleEnum").into_ir_type(db).source_type_owned().into_mut(),
    }

    cast_snapshot! {
        name: enum_out,
        from: |db| db.item_id("ExampleEnum").into_ir_type(db).source_type_owned(),
        to: |db| db.item_id("ExampleEnum").into_ir_type(db).extern_out(),
    }
}
