//! JavaScript-specific utilities for compiling facade casts as statements.

use crate::{
    compiler::{
        codegen::{
            cast::CastContinuation,
            fragment::{FragmentOnce, Printer},
        },
        db::Db,
    },
    define_cast_graph, frag,
};

define_cast_graph! {
    pub FacadeCast<'db> {
        types {
            FacadeType(crate::compiler::ir::facade::FacadeType<'db>),
            FacadeFfiType(crate::compiler::codegen::js::types::FacadeFfiType<'db>),
        }
        direct_rules {
            FacadeClassTypeToFfiClassType {
                rule: crate::compiler::codegen::js::classes::CastFacadeClassTypeToFfiClassTypeRule,
                from: FacadeType,
                to: FacadeFfiType,
            }
            FfiClassTypeToFacadeClassType {
                rule: crate::compiler::codegen::js::classes::CastFfiClassTypeToFacadeClassTypeRule,
                from: FacadeFfiType,
                to: FacadeType,
            }
        }
        indirect_rules {

        }
    }
}

/// A JavaScript cast request whose converted value is bound to `out_var_name`.
pub struct Cast<From, To, InExpr, OutExpr> {
    pub from: From,
    pub to: To,
    pub in_expr: InExpr,
    pub out_var_name: OutExpr,
}

/// Compiles facade casts into JavaScript statements.
pub trait FacadeCastCompiler<'db> {
    /// Compiles one cast into a named JavaScript variable before `inner_stmt`.
    fn compile_cast_stmt<From, To, InExpr, OutExpr, Continue>(
        self,
        db: &'db dyn Db,
        cast: Cast<From, To, InExpr, OutExpr>,
        continuation: Continue,
    ) -> impl FragmentOnce + 'db
    where
        From: Into<FacadeCast::CastType<'db>> + 'db,
        To: Into<FacadeCast::CastType<'db>> + 'db,
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
        From: Into<FacadeCast::CastType<'db>> + 'db,
        To: Into<FacadeCast::CastType<'db>> + 'db,
        InExpr: FragmentOnce + 'db,
        OutExpr: FragmentOnce + 'db,
        Continue: FragmentOnce + 'db;
}

fn compile_cast_stmt<'db, From, To, InExpr, OutExpr, Continue>(
    db: &'db dyn Db,
    cast: Cast<From, To, InExpr, OutExpr>,
    continuation: Continue,
) -> impl FragmentOnce + 'db
where
    From: Into<FacadeCast::CastType<'db>> + 'db,
    To: Into<FacadeCast::CastType<'db>> + 'db,
    InExpr: FragmentOnce + 'db,
    OutExpr: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    FacadeCast::compile_cast(
        db,
        cast.from,
        cast.to,
        cast.in_expr,
        BindCastOutput {
            out_var_name: cast.out_var_name,
            continue_with: continuation,
        },
    )
}

fn compile_casts_stmt<'db, Casts, From, To, InExpr, OutExpr, Continue>(
    db: &'db dyn Db,
    casts: Casts,
    inner_stmt: Continue,
) -> impl FragmentOnce + 'db
where
    Casts: IntoIterator<Item = Cast<From, To, InExpr, OutExpr>> + 'db,
    Casts::IntoIter: 'db,
    From: Into<FacadeCast::CastType<'db>> + 'db,
    To: Into<FacadeCast::CastType<'db>> + 'db,
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

/// Sequences JavaScript variable casts while keeping each output in scope.
struct CompileCastsStmt<'db, Casts, Continue> {
    db: &'db dyn Db,
    casts: Casts,
    inner_stmt: Continue,
}

impl<'db, Casts, From, To, InExpr, OutExpr, Continue> FragmentOnce
    for CompileCastsStmt<'db, Casts, Continue>
where
    Casts: Iterator<Item = Cast<From, To, InExpr, OutExpr>> + 'db,
    From: Into<FacadeCast::CastType<'db>> + 'db,
    To: Into<FacadeCast::CastType<'db>> + 'db,
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

impl<'db> FacadeCastCompiler<'db> for FacadeCast::Graph {
    fn compile_cast_stmt<From, To, InExpr, OutExpr, Continue>(
        self,
        db: &'db dyn Db,
        cast: Cast<From, To, InExpr, OutExpr>,
        inner_stmt: Continue,
    ) -> impl FragmentOnce + 'db
    where
        From: Into<FacadeCast::CastType<'db>> + 'db,
        To: Into<FacadeCast::CastType<'db>> + 'db,
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
        From: Into<FacadeCast::CastType<'db>> + 'db,
        To: Into<FacadeCast::CastType<'db>> + 'db,
        InExpr: FragmentOnce + 'db,
        OutExpr: FragmentOnce + 'db,
        Continue: FragmentOnce + 'db,
    {
        compile_casts_stmt(db, casts, inner_stmt)
    }
}

/// Binds a cast result before compiling the remaining JavaScript statements.
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
            var ~{out_var_name}~ = ~{out_expr};@
            {continue_with}
        )
    }
}
