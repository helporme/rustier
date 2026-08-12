//! Rust-specific utilities for compiling casts used by generated shim.

use crate::{
    compiler::{
        codegen::{
            cast::CastContinuation,
            fragment::{FragmentOnce, Printer, fragment},
        },
        db::Db,
    },
    define_cast_graph, frag,
};

define_cast_graph! {
    pub ShimCast<'db> {
        types {
            Type(crate::compiler::ir::rust::Type<'db>),
            ShimType(crate::compiler::ir::shim::ShimType<'db>),
            TetheringStructType(crate::compiler::codegen::shim::tethering::TetheringStructType<'db>),
        }
        direct_rules {
            TetheringStructTypeToClassShimType {
                rule: crate::compiler::codegen::shim::classes::CastTetheringStructTypeToClassShimType,
                from: TetheringStructType,
                to: ShimType,
            }
            ClassShimTypeToTetheringStructType {
                rule: crate::compiler::codegen::shim::classes::CastClassShimTypeToTetheringStructType,
                from: ShimType,
                to: TetheringStructType,
            }
            JsAnyToF64 {
                rule: crate::compiler::codegen::shim::primitives::CastJsAnyToF64Rule,
                from: ShimType,
                to: Type,
            }
            JsAnyToBool {
                rule: crate::compiler::codegen::shim::primitives::CastJsAnyToBoolRule,
                from: ShimType,
                to: Type,
            }
            JsAnyToString {
                rule: crate::compiler::codegen::shim::primitives::CastJsAnyToStringRule,
                from: ShimType,
                to: Type,
            }
            F64ToSignedInt {
                rule: crate::compiler::codegen::shim::primitives::CastF64ToSignedIntRule,
                from: Type,
                to: Type,
            }
            F64ToUnsignedInt {
                rule: crate::compiler::codegen::shim::primitives::CastF64ToUnsignedIntRule,
                from: Type,
                to: Type,
            }
            U32ToChar {
                rule: crate::compiler::codegen::shim::primitives::CastU32ToCharRule,
                from: Type,
                to: Type,
            }
            F64ToF32 {
                rule: crate::compiler::codegen::shim::primitives::CastF64ToF32Rule,
                from: Type,
                to: Type,
            }
            CharToU32Boundary {
                rule: crate::compiler::codegen::shim::primitives::CastCharToU32BoundaryRule,
                from: Type,
                to: ShimType,
            }
            F32ToF64Boundary {
                rule: crate::compiler::codegen::shim::primitives::CastF32ToF64BoundaryRule,
                from: Type,
                to: ShimType,
            }
            PrimitiveToSameBoundary {
                rule: crate::compiler::codegen::shim::primitives::CastPrimitiveToSameBoundaryRule,
                from: Type,
                to: ShimType,
            }
            TypeToTetheringStructType {
                rule: crate::compiler::codegen::shim::tethering::CastTypeToTetheringStructType,
                from: Type,
                to: TetheringStructType,
            }
        }
        indirect_rules {
            OwnedTypeToBorrowedType {
                rule: crate::compiler::codegen::shim::types::CastOwnedTypeToBorrowedTypeRule,
                from: Type,
                to: Type,
            }
            StringToStrRef {
                rule: crate::compiler::codegen::shim::primitives::CastStringToStrRefRule,
                from: Type,
                to: Type,
            }
            StringToStrMut {
                rule: crate::compiler::codegen::shim::primitives::CastStringToStrMutRule,
                from: Type,
                to: Type,
            }
            TetheringStructTypeToBorrowedType {
                rule: crate::compiler::codegen::shim::tethering::CastTetheringStructTypeToBorrowedType,
                from: TetheringStructType,
                to: Type,
            }
            TetheringStructTypeToOwnedType {
                rule: crate::compiler::codegen::shim::tethering::CastTetheringStructTypeToOwnedType,
                from: TetheringStructType,
                to: Type,
            }
        }
    }
}

/// A Rust cast request whose converted value is bound to `out_var_name`.
pub struct Cast<From, To, InExpr, OutExpr> {
    pub from: From,
    pub to: To,
    pub in_expr: InExpr,
    pub out_var_name: OutExpr,
}

/// Compiles cast fragments for generated Rust Shim.
pub trait ShimCastCompiler<'db> {
    /// Compiles one cast into a named Rust binding before `inner_stmt`.
    fn compile_cast_stmt<From, To, InExpr, OutExpr, Continue>(
        self,
        db: &'db dyn Db,
        cast: Cast<From, To, InExpr, OutExpr>,
        inner_stmt: Continue,
    ) -> impl FragmentOnce + 'db
    where
        From: Into<ShimCast::CastType<'db>> + 'db,
        To: Into<ShimCast::CastType<'db>> + 'db,
        InExpr: FragmentOnce + 'db,
        OutExpr: FragmentOnce + 'db,
        Continue: FragmentOnce + 'db;

    /// Compiles multiple named casts in order before `inner_stmt`.
    fn compile_casts_stmt<Casts, From, To, InExpr, OutExpr, Continue>(
        self,
        db: &'db dyn Db,
        casts: Casts,
        inner_stmt: Continue,
    ) -> impl FragmentOnce + 'db
    where
        Casts: IntoIterator<Item = Cast<From, To, InExpr, OutExpr>> + 'db,
        Casts::IntoIter: 'db,
        From: Into<ShimCast::CastType<'db>> + 'db,
        To: Into<ShimCast::CastType<'db>> + 'db,
        InExpr: FragmentOnce + 'db,
        OutExpr: FragmentOnce + 'db,
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
        From: Into<ShimCast::CastType<'db>> + 'db,
        To: Into<ShimCast::CastType<'db>> + 'db,
        Expr: FragmentOnce + 'db;

    /// Compiles named casts around a Rust expression and captures its result
    /// only when an indirect cast scopes the expression.
    fn compile_casts_expr<Casts, From, To, InExpr, OutExpr, Inner>(
        self,
        db: &'db dyn Db,
        casts: Casts,
        inner_expr: Inner,
    ) -> impl FragmentOnce + 'db
    where
        Casts: IntoIterator<Item = Cast<From, To, InExpr, OutExpr>> + 'db,
        Casts::IntoIter: 'db,
        From: Into<ShimCast::CastType<'db>> + 'db,
        To: Into<ShimCast::CastType<'db>> + 'db,
        InExpr: FragmentOnce + 'db,
        OutExpr: FragmentOnce + 'db,
        Inner: FragmentOnce + 'db;
}

/// Compiles one cast into a named Rust binding before `inner_stmt`.
pub fn compile_cast_stmt<'db, From, To, InExpr, OutExpr, Continue>(
    db: &'db dyn Db,
    cast: Cast<From, To, InExpr, OutExpr>,
    inner_stmt: Continue,
) -> impl FragmentOnce + 'db
where
    From: Into<ShimCast::CastType<'db>> + 'db,
    To: Into<ShimCast::CastType<'db>> + 'db,
    InExpr: FragmentOnce + 'db,
    OutExpr: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    ShimCast::compile_cast(
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
pub fn compile_casts_stmt<'db, Casts, From, To, InExpr, OutExpr, Continue>(
    db: &'db dyn Db,
    casts: Casts,
    inner_stmt: Continue,
) -> impl FragmentOnce + 'db
where
    Casts: IntoIterator<Item = Cast<From, To, InExpr, OutExpr>> + 'db,
    Casts::IntoIter: 'db,
    From: Into<ShimCast::CastType<'db>> + 'db,
    To: Into<ShimCast::CastType<'db>> + 'db,
    InExpr: FragmentOnce + 'db,
    OutExpr: FragmentOnce + 'db,
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

impl<'db, Casts, From, To, InExpr, OutExpr, Continue> FragmentOnce
    for CompileCastsStmt<'db, Casts, Continue>
where
    Casts: Iterator<Item = Cast<From, To, InExpr, OutExpr>> + 'db,
    From: Into<ShimCast::CastType<'db>> + 'db,
    To: Into<ShimCast::CastType<'db>> + 'db,
    InExpr: FragmentOnce + 'db,
    OutExpr: FragmentOnce + 'db,
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
    From: Into<ShimCast::CastType<'db>> + 'db,
    To: Into<ShimCast::CastType<'db>> + 'db,
    Expr: FragmentOnce + 'db,
{
    let from = from.into();
    let to = to.into();
    let uses_indirect_rule = ShimCast::find_cast_path(db, from.clone(), to.clone())
        .iter()
        .any(|step| step.rule.is_indirect());

    fragment(move |printer| {
        if uses_indirect_rule {
            frag!({{
                let mut __cast_expr_result = None;@
                {ShimCast::compile_cast(
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
            ShimCast::compile_cast(db, from, to, expr, ReturnCastOutput).apply(printer);
        }
    })
}

/// Compiles named casts around a Rust expression and captures its result only
/// when an indirect cast scopes the expression.
pub fn compile_casts_expr<'db, Casts, From, To, InExpr, OutExpr, Inner>(
    db: &'db dyn Db,
    casts: Casts,
    inner_expr: Inner,
) -> impl FragmentOnce + 'db
where
    Casts: IntoIterator<Item = Cast<From, To, InExpr, OutExpr>> + 'db,
    Casts::IntoIter: 'db,
    From: Into<ShimCast::CastType<'db>> + 'db,
    To: Into<ShimCast::CastType<'db>> + 'db,
    InExpr: FragmentOnce + 'db,
    OutExpr: FragmentOnce + 'db,
    Inner: FragmentOnce + 'db,
{
    CompileCastsExpr {
        db,
        casts: casts.into_iter(),
        inner_expr,
    }
}

impl<'db> ShimCastCompiler<'db> for ShimCast::Graph {
    fn compile_cast_stmt<From, To, InExpr, OutExpr, Continue>(
        self,
        db: &'db dyn Db,
        cast: Cast<From, To, InExpr, OutExpr>,
        inner_stmt: Continue,
    ) -> impl FragmentOnce + 'db
    where
        From: Into<ShimCast::CastType<'db>> + 'db,
        To: Into<ShimCast::CastType<'db>> + 'db,
        InExpr: FragmentOnce + 'db,
        OutExpr: FragmentOnce + 'db,
        Continue: FragmentOnce + 'db,
    {
        compile_cast_stmt(db, cast, inner_stmt)
    }

    fn compile_casts_stmt<Casts, From, To, InExpr, OutExpr, Continue>(
        self,
        db: &'db dyn Db,
        casts: Casts,
        inner_stmt: Continue,
    ) -> impl FragmentOnce + 'db
    where
        Casts: IntoIterator<Item = Cast<From, To, InExpr, OutExpr>> + 'db,
        Casts::IntoIter: 'db,
        From: Into<ShimCast::CastType<'db>> + 'db,
        To: Into<ShimCast::CastType<'db>> + 'db,
        InExpr: FragmentOnce + 'db,
        OutExpr: FragmentOnce + 'db,
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
        From: Into<ShimCast::CastType<'db>> + 'db,
        To: Into<ShimCast::CastType<'db>> + 'db,
        Expr: FragmentOnce + 'db,
    {
        compile_cast_expr(db, from, to, expr)
    }

    fn compile_casts_expr<Casts, From, To, InExpr, OutExpr, Inner>(
        self,
        db: &'db dyn Db,
        casts: Casts,
        inner_expr: Inner,
    ) -> impl FragmentOnce + 'db
    where
        Casts: IntoIterator<Item = Cast<From, To, InExpr, OutExpr>> + 'db,
        Casts::IntoIter: 'db,
        From: Into<ShimCast::CastType<'db>> + 'db,
        To: Into<ShimCast::CastType<'db>> + 'db,
        InExpr: FragmentOnce + 'db,
        OutExpr: FragmentOnce + 'db,
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

impl<'db, Casts, From, To, InExpr, OutExpr, Inner> FragmentOnce
    for CompileCastsExpr<'db, Casts, Inner>
where
    Casts: Iterator<Item = Cast<From, To, InExpr, OutExpr>> + 'db,
    From: Into<ShimCast::CastType<'db>> + 'db,
    To: Into<ShimCast::CastType<'db>> + 'db,
    InExpr: FragmentOnce + 'db,
    OutExpr: FragmentOnce + 'db,
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

        let uses_indirect_rule = ShimCast::find_cast_path(self.db, from.clone(), to.clone())
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
struct BindCastOutput<OutExpr, Continue> {
    out_var_name: OutExpr,
    continue_with: Continue,
}

impl<'db, OutExpr, Continue> CastContinuation<'db> for BindCastOutput<OutExpr, Continue>
where
    OutExpr: FragmentOnce + 'db,
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
