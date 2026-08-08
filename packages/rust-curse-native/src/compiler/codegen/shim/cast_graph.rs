//! Rust-specific utilities for compiling casts used by generated shim.

use crate::{
    compiler::{
        codegen::{
            cast::CastContinuation,
            fragment::{FragmentOnce, Printer, fragment},
            shim::{
                class::{
                    CastClassShimTypeToTetheringStructType, CastTetheringStructTypeToClassShimType,
                },
                tethering::TetheringStructType,
            },
        },
        db::Db,
        ir::{rust::Type, shim::ShimType},
    },
    define_cast_graph, frag,
};

type X = CastTetheringStructTypeToClassShimType;

define_cast_graph! {
    pub ShimCast<'db> {
        types {
            Type(Type<'db>),
            ShimType(ShimType<'db>),
            TetheringStructType(TetheringStructType<'db>),
        }
        direct_rules {
            TetheringStructTypeToClassShimType {
                rule: CastTetheringStructTypeToClassShimType,
                from: TetheringStructType,
                to: ShimType,
            }
            ClassShimTypeToTetheringStructType {
                rule: CastClassShimTypeToTetheringStructType,
                from: ShimType,
                to: TetheringStructType,
            }
        }
        indirect_rules { }
    }
}

// define_cast_graph! {
//     pub ShimCast<'db> {
//         types {
//             BoundaryType(ExternType<'db>),
//             Type(Type<'db>),
//             TetheringStructType(TetheringStructType<'db>),
//         }

//         direct_rules {
//             JsAnyToF64 {
//                 rule: CastJsAnyToF64Rule,
//                 from: BoundaryType,
//                 to: Type,
//             }
//             JsAnyToBool {
//                 rule: CastJsAnyToBoolRule,
//                 from: BoundaryType,
//                 to: Type,
//             }
//             JsAnyToString {
//                 rule: CastJsAnyToStringRule,
//                 from: BoundaryType,
//                 to: Type,
//             }
//             F64ToSignedInt {
//                 rule: CastF64ToSignedIntRule,
//                 from: Type,
//                 to: Type,
//             }
//             F64ToUnsignedInt {
//                 rule: CastF64ToUnsignedIntRule,
//                 from: Type,
//                 to: Type,
//             }
//             U32ToChar {
//                 rule: CastU32ToCharRule,
//                 from: Type,
//                 to: Type,
//             }
//             F64ToF32 {
//                 rule: CastF64ToF32Rule,
//                 from: Type,
//                 to: Type,
//             }
//             CharToU32Boundary {
//                 rule: CastCharToU32BoundaryRule,
//                 from: Type,
//                 to: BoundaryType,
//             }
//             F32ToF64Boundary {
//                 rule: CastF32ToF64BoundaryRule,
//                 from: Type,
//                 to: BoundaryType,
//             }
//             PrimitiveToSameBoundary {
//                 rule: CastPrimitiveToSameBoundaryRule,
//                 from: Type,
//                 to: BoundaryType,
//             }
//             TypeToTetheringStructType {
//                 rule: CastTypeToTetheringStructType,
//                 from: Type,
//                 to: TetheringStructType,
//             }
//             ClassBoundaryTypeToTetheringStructType {
//                 rule: CastClassExternTypeToTetheringStructType,
//                 from: BoundaryType,
//                 to: TetheringStructType
//             }
//             TetheringStructTypeToClassBoundaryType {
//                 rule: CastTetheringStructTypeToClassExternType,
//                 from: TetheringStructType,
//                 to: BoundaryType
//             }
//         }

//         indirect_rules {
//             OwnedTypeToBorrowedType {
//                 rule: CastOwnedTypeToBorrowedTypeRule,
//                 from: Type,
//                 to: Type,
//             }
//             StringToStrRef {
//                 rule: CastStringToStrRefRule,
//                 from: Type,
//                 to: Type,
//             }
//             StringToStrMut {
//                 rule: CastStringToStrMutRule,
//                 from: Type,
//                 to: Type,
//             }
//             TetheringStructTypeToBorrowedType {
//                 rule: CastTetheringStructTypeToBorrowedType,
//                 from: TetheringStructType,
//                 to: Type,
//             }
//             TetheringStructTypeToOwnedType {
//                 rule: CastTetheringStructTypeToOwnedType,
//                 from: TetheringStructType,
//                 to: Type,
//             }
//         }
//     }
// }

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

// /// A rule to cast an owned source type to a reference source type.
// pub struct CastOwnedTypeToBorrowedTypeRule;

// impl<'db> IndirectCastRule<'db, Type<'db>, Type<'db>> for
// CastOwnedTypeToBorrowedTypeRule {     fn predecessor(self, _db: &'db dyn Db,
// to: Type<'db>) -> Option<Type<'db>> {         match to {
//             Type::Borrowed { inner, .. } if
// Type::Value(inner).allows_generic_borrowing_cast() => {
// Some(Type::Value(inner))             }
//             _ => None,
//         }
//     }

//     fn render<'fragment, Expr, Continue>(
//         self,
//         _db: &'db dyn Db,
//         _from: Type<'db>,
//         to: Type<'db>,
//         in_expr: Expr,
//         continue_with: Continue,
//     ) -> impl FragmentOnce + 'fragment
//     where
//         'db: 'fragment,
//         Expr: FragmentOnce + 'fragment,
//         Continue: CastContinuation<'db>,
//     {
//         let Type::Borrowed { is_mutable, .. } = to else {
//             unreachable!()
//         };

//         let borrowing = if is_mutable {
//             frag!(&mut __cast_owned_value)
//         } else {
//             frag!(&__cast_owned_value)
//         };
//         let continue_with = continue_with.compile(borrowing);

//         frag!(
//             let mut __cast_owned_value = {in_expr};
//             {continue_with}
//         )
//     }
// }

// #[cfg(test)]
// pub mod test {
//     use std::fmt::{Debug, Write as _};

//     use crate::{
//         compiler::{
//             codegen::Shim::cast_graph::ShimCastCompiler,
//             codegen::fragment::{FragmentOnce, Printer, fragment},
//         },
//         frag,
//     };

//     /// Compiles visible fake cast expressions for testing binding code
//     /// generation.
//     #[derive(Copy, Clone)]
//     pub struct TestCastCompiler;

//     struct DebugString<T>(T);

//     impl<T> FragmentOnce for DebugString<T>
//     where
//         T: Debug,
//     {
//         fn apply(self, printer: &mut Printer) {
//             let debug = format!("{:?}", self.0);
//             write!(printer, "{debug:?}").expect("writing to source printer
// cannot fail");         }
//     }

//     fn compile_test_cast_expr<'db, From, To, Expr>(
//         from: From,
//         to: To,
//         expr: Expr,
//     ) -> impl FragmentOnce + 'db
//     where
//         From: Into<super::ShimCast::CastType<'db>> + 'db,
//         To: Into<super::ShimCast::CastType<'db>> + 'db,
//         Expr: FragmentOnce + 'db,
//     {
//         let from = DebugString(from.into());
//         let to = DebugString(to.into());

//         frag!(cast.from({ from }).to({ to }).an({ expr }))
//     }

//     impl<'db> ShimCastCompiler<'db> for TestCastCompiler {
//         fn compile_cast_stmt<From, To, InExpr, OutExpr, Continue>(
//             self,
//             _db: &'db dyn crate::compiler::db::Db,
//             cast: super::Cast<From, To, InExpr, OutExpr>,
//             inner_stmt: Continue,
//         ) -> impl FragmentOnce + 'db
//         where
//             From: Into<super::ShimCast::CastType<'db>> + 'db,
//             To: Into<super::ShimCast::CastType<'db>> + 'db,
//             InExpr: FragmentOnce + 'db,
//             OutExpr: FragmentOnce + 'db,
//             Continue: FragmentOnce + 'db,
//         {
//             let cast_expr = compile_test_cast_expr(cast.from, cast.to,
// cast.in_expr);

//             frag!(
//                 let ~{cast.out_var_name}~ = {cast_expr};@
//                 {inner_stmt}
//             )
//         }

//         fn compile_casts_stmt<Casts, From, To, InExpr, OutExpr, Continue>(
//             self,
//             _db: &'db dyn crate::compiler::db::Db,
//             casts: Casts,
//             inner_stmt: Continue,
//         ) -> impl FragmentOnce + 'db
//         where
//             Casts: IntoIterator<Item = super::Cast<From, To, InExpr,
// OutExpr>> + 'db,             Casts::IntoIter: 'db,
//             From: Into<super::ShimCast::CastType<'db>> + 'db,
//             To: Into<super::ShimCast::CastType<'db>> + 'db,
//             InExpr: FragmentOnce + 'db,
//             OutExpr: FragmentOnce + 'db,
//             Continue: FragmentOnce + 'db,
//         {
//             fragment(move |printer| {
//                 for cast in casts {
//                     let cast_expr = compile_test_cast_expr(cast.from,
// cast.to, cast.in_expr);                     frag!(let ~{cast.out_var_name}~ =
// {cast_expr};@).apply(printer);                 }

//                 inner_stmt.apply(printer);
//             })
//         }

//         fn compile_cast_expr<From, To, Expr>(
//             self,
//             _db: &'db dyn crate::compiler::db::Db,
//             from: From,
//             to: To,
//             expr: Expr,
//         ) -> impl FragmentOnce + 'db
//         where
//             From: Into<super::ShimCast::CastType<'db>> + 'db,
//             To: Into<super::ShimCast::CastType<'db>> + 'db,
//             Expr: FragmentOnce + 'db,
//         {
//             compile_test_cast_expr(from, to, expr)
//         }

//         fn compile_casts_expr<Casts, From, To, InExpr, OutExpr, Inner>(
//             self,
//             _db: &'db dyn crate::compiler::db::Db,
//             casts: Casts,
//             inner_expr: Inner,
//         ) -> impl FragmentOnce + 'db
//         where
//             Casts: IntoIterator<Item = super::Cast<From, To, InExpr,
// OutExpr>> + 'db,             Casts::IntoIter: 'db,
//             From: Into<super::ShimCast::CastType<'db>> + 'db,
//             To: Into<super::ShimCast::CastType<'db>> + 'db,
//             InExpr: FragmentOnce + 'db,
//             OutExpr: FragmentOnce + 'db,
//             Inner: FragmentOnce + 'db,
//         {
//             fragment(move |printer| {
//                 let mut casts = casts.into_iter();
//                 let Some(first_cast) = casts.next() else {
//                     inner_expr.apply(printer);
//                     return;
//                 };
//                 let cast_exprs = fragment(move |printer| {
//                     for cast in std::iter::once(first_cast).chain(casts) {
//                         let cast_expr = compile_test_cast_expr(cast.from,
// cast.to, cast.in_expr);                         frag!(let
// ~{cast.out_var_name}~ = {cast_expr};@).apply(printer);                     }

//                     inner_expr.apply(printer);
//                 });

//                 frag!({ { { cast_exprs } } }).apply(printer);
//             })
//         }
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::{Cast, compile_cast_stmt};
//     use crate::{
//         assert_rust_snap,
//         compiler::{
//             codegen::fragment::FragmentOnce as _,
//             ir::Type,
//             ir::{IntoIrTypeExt as _, ValueType},
//         },
//         frag,
//     };

//     macro_rules! cast_snapshot {
//         (
//             name: $name:ident,
//             from: |$from_db:ident| $from:expr,
//             to: |$to_db:ident| $to:expr $(,)?
//         ) => {
//             #[test]
//             fn $name() {
//                 assert_rust_snap! {
//                     source: {
//                         #[derive(Clone)]
//                         pub struct ExampleStruct;

//                         #[derive(Clone)]
//                         pub enum ExampleEnum {
//                             Variant,
//                         }
//                     },
//                     expression: |__db, printer| {
//                         let __db = &*__db;
//                         let from = {
//                             let $from_db = __db;
//                             $from
//                         };
//                         let to = {
//                             let $to_db = __db;
//                             $to
//                         };

//                         frag!(
//                             fn cast(input: ())~{{
//                                 {compile_cast_stmt(
//                                     __db,
//                                     Cast {
//                                         from,
//                                         to,
//                                         in_expr: "input",
//                                         out_var_name: "output",
//                                     },
//                                     frag!(consume(output);),
//                                 )}
//                             }}
//                         )
//                         .apply(printer);
//                     },
//                 }
//             }
//         };
//     }

//     cast_snapshot! {
//         name: bool_in,
//         from: |_db| ValueType::bool.extern_in(),
//         to: |_db| Type::Value(ValueType::bool),
//     }

//     cast_snapshot! {
//         name: bool_ref_in,
//         from: |_db| ValueType::bool.extern_in(),
//         to: |_db| Type::Value(ValueType::bool).into_ref(),
//     }

//     cast_snapshot! {
//         name: bool_mut_in,
//         from: |_db| ValueType::bool.extern_in(),
//         to: |_db| Type::Value(ValueType::bool).into_mut(),
//     }

//     cast_snapshot! {
//         name: bool_out,
//         from: |_db| Type::Value(ValueType::bool),
//         to: |_db| ValueType::bool.extern_out(),
//     }

//     cast_snapshot! {
//         name: char_in,
//         from: |_db| ValueType::char.extern_in(),
//         to: |_db| Type::Value(ValueType::char),
//     }

//     cast_snapshot! {
//         name: char_ref_in,
//         from: |_db| ValueType::char.extern_in(),
//         to: |_db| Type::Value(ValueType::char).into_ref(),
//     }

//     cast_snapshot! {
//         name: char_mut_in,
//         from: |_db| ValueType::char.extern_in(),
//         to: |_db| Type::Value(ValueType::char).into_mut(),
//     }

//     cast_snapshot! {
//         name: char_out,
//         from: |_db| Type::Value(ValueType::char),
//         to: |_db| ValueType::char.extern_out(),
//     }

//     cast_snapshot! {
//         name: i8_in,
//         from: |_db| ValueType::i8.extern_in(),
//         to: |_db| Type::Value(ValueType::i8),
//     }

//     cast_snapshot! {
//         name: i8_ref_in,
//         from: |_db| ValueType::i8.extern_in(),
//         to: |_db| Type::Value(ValueType::i8).into_ref(),
//     }

//     cast_snapshot! {
//         name: i8_mut_in,
//         from: |_db| ValueType::i8.extern_in(),
//         to: |_db| Type::Value(ValueType::i8).into_mut(),
//     }

//     cast_snapshot! {
//         name: i8_out,
//         from: |_db| Type::Value(ValueType::i8),
//         to: |_db| ValueType::i8.extern_out(),
//     }

//     cast_snapshot! {
//         name: u8_in,
//         from: |_db| ValueType::u8.extern_in(),
//         to: |_db| Type::Value(ValueType::u8),
//     }

//     cast_snapshot! {
//         name: u8_ref_in,
//         from: |_db| ValueType::u8.extern_in(),
//         to: |_db| Type::Value(ValueType::u8).into_ref(),
//     }

//     cast_snapshot! {
//         name: u8_mut_in,
//         from: |_db| ValueType::u8.extern_in(),
//         to: |_db| Type::Value(ValueType::u8).into_mut(),
//     }

//     cast_snapshot! {
//         name: u8_out,
//         from: |_db| Type::Value(ValueType::u8),
//         to: |_db| ValueType::u8.extern_out(),
//     }

//     cast_snapshot! {
//         name: i16_in,
//         from: |_db| ValueType::i16.extern_in(),
//         to: |_db| Type::Value(ValueType::i16),
//     }

//     cast_snapshot! {
//         name: i16_ref_in,
//         from: |_db| ValueType::i16.extern_in(),
//         to: |_db| Type::Value(ValueType::i16).into_ref(),
//     }

//     cast_snapshot! {
//         name: i16_mut_in,
//         from: |_db| ValueType::i16.extern_in(),
//         to: |_db| Type::Value(ValueType::i16).into_mut(),
//     }

//     cast_snapshot! {
//         name: i16_out,
//         from: |_db| Type::Value(ValueType::i16),
//         to: |_db| ValueType::i16.extern_out(),
//     }

//     cast_snapshot! {
//         name: u16_in,
//         from: |_db| ValueType::u16.extern_in(),
//         to: |_db| Type::Value(ValueType::u16),
//     }

//     cast_snapshot! {
//         name: u16_ref_in,
//         from: |_db| ValueType::u16.extern_in(),
//         to: |_db| Type::Value(ValueType::u16).into_ref(),
//     }

//     cast_snapshot! {
//         name: u16_mut_in,
//         from: |_db| ValueType::u16.extern_in(),
//         to: |_db| Type::Value(ValueType::u16).into_mut(),
//     }

//     cast_snapshot! {
//         name: u16_out,
//         from: |_db| Type::Value(ValueType::u16),
//         to: |_db| ValueType::u16.extern_out(),
//     }

//     cast_snapshot! {
//         name: i32_in,
//         from: |_db| ValueType::i32.extern_in(),
//         to: |_db| Type::Value(ValueType::i32),
//     }

//     cast_snapshot! {
//         name: i32_ref_in,
//         from: |_db| ValueType::i32.extern_in(),
//         to: |_db| Type::Value(ValueType::i32).into_ref(),
//     }

//     cast_snapshot! {
//         name: i32_mut_in,
//         from: |_db| ValueType::i32.extern_in(),
//         to: |_db| Type::Value(ValueType::i32).into_mut(),
//     }

//     cast_snapshot! {
//         name: i32_out,
//         from: |_db| Type::Value(ValueType::i32),
//         to: |_db| ValueType::i32.extern_out(),
//     }

//     cast_snapshot! {
//         name: u32_in,
//         from: |_db| ValueType::u32.extern_in(),
//         to: |_db| Type::Value(ValueType::u32),
//     }

//     cast_snapshot! {
//         name: u32_ref_in,
//         from: |_db| ValueType::u32.extern_in(),
//         to: |_db| Type::Value(ValueType::u32).into_ref(),
//     }

//     cast_snapshot! {
//         name: u32_mut_in,
//         from: |_db| ValueType::u32.extern_in(),
//         to: |_db| Type::Value(ValueType::u32).into_mut(),
//     }

//     cast_snapshot! {
//         name: u32_out,
//         from: |_db| Type::Value(ValueType::u32),
//         to: |_db| ValueType::u32.extern_out(),
//     }

//     cast_snapshot! {
//         name: f32_in,
//         from: |_db| ValueType::f32.extern_in(),
//         to: |_db| Type::Value(ValueType::f32),
//     }

//     cast_snapshot! {
//         name: f32_ref_in,
//         from: |_db| ValueType::f32.extern_in(),
//         to: |_db| Type::Value(ValueType::f32).into_ref(),
//     }

//     cast_snapshot! {
//         name: f32_mut_in,
//         from: |_db| ValueType::f32.extern_in(),
//         to: |_db| Type::Value(ValueType::f32).into_mut(),
//     }

//     cast_snapshot! {
//         name: f32_out,
//         from: |_db| Type::Value(ValueType::f32),
//         to: |_db| ValueType::f32.extern_out(),
//     }

//     cast_snapshot! {
//         name: f64_in,
//         from: |_db| ValueType::f64.extern_in(),
//         to: |_db| Type::Value(ValueType::f64),
//     }

//     cast_snapshot! {
//         name: f64_ref_in,
//         from: |_db| ValueType::f64.extern_in(),
//         to: |_db| Type::Value(ValueType::f64).into_ref(),
//     }

//     cast_snapshot! {
//         name: f64_mut_in,
//         from: |_db| ValueType::f64.extern_in(),
//         to: |_db| Type::Value(ValueType::f64).into_mut(),
//     }

//     cast_snapshot! {
//         name: f64_out,
//         from: |_db| Type::Value(ValueType::f64),
//         to: |_db| ValueType::f64.extern_out(),
//     }

//     cast_snapshot! {
//         name: str_in,
//         from: |_db| ValueType::Str.extern_in(),
//         to: |_db| Type::Value(ValueType::Str).into_ref(),
//     }

//     cast_snapshot! {
//         name: str_ref_in,
//         from: |_db| ValueType::Str.extern_in(),
//         to: |_db| Type::Value(ValueType::Str).into_ref(),
//     }

//     cast_snapshot! {
//         name: str_mut_in,
//         from: |_db| ValueType::Str.extern_in(),
//         to: |_db| Type::Value(ValueType::Str).into_mut(),
//     }

//     cast_snapshot! {
//         name: string_in,
//         from: |_db| ValueType::String.extern_in(),
//         to: |_db| Type::Value(ValueType::String),
//     }

//     cast_snapshot! {
//         name: string_ref_in,
//         from: |_db| ValueType::String.extern_in(),
//         to: |_db| Type::Value(ValueType::String).into_ref(),
//     }

//     cast_snapshot! {
//         name: string_mut_in,
//         from: |_db| ValueType::String.extern_in(),
//         to: |_db| Type::Value(ValueType::String).into_mut(),
//     }

//     cast_snapshot! {
//         name: string_out,
//         from: |_db| Type::Value(ValueType::String),
//         to: |_db| ValueType::String.extern_out(),
//     }

//     cast_snapshot! {
//         name: struct_in,
//         from: |db|
// db.item_id("ExampleStruct").into_value_type(db).extern_in(),         to: |db|
// Type::Value(db.item_id("ExampleStruct").into_value_type(db)),     }

//     cast_snapshot! {
//         name: struct_ref_in,
//         from: |db|
// db.item_id("ExampleStruct").into_value_type(db).extern_in(),         to: |db|
// Type::Value(db.item_id("ExampleStruct").into_value_type(db)).into_ref(),
//     }

//     cast_snapshot! {
//         name: struct_mut_in,
//         from: |db|
// db.item_id("ExampleStruct").into_value_type(db).extern_in(),         to: |db|
// Type::Value(db.item_id("ExampleStruct").into_value_type(db)).into_mut(),
//     }

//     cast_snapshot! {
//         name: struct_out,
//         from: |db|
// Type::Value(db.item_id("ExampleStruct").into_value_type(db)),         to:
// |db| db.item_id("ExampleStruct").into_value_type(db).extern_out(),     }

//     cast_snapshot! {
//         name: enum_in,
//         from: |db| db.item_id("ExampleEnum").into_value_type(db).extern_in(),
//         to: |db| Type::Value(db.item_id("ExampleEnum").into_value_type(db)),
//     }

//     cast_snapshot! {
//         name: enum_ref_in,
//         from: |db| db.item_id("ExampleEnum").into_value_type(db).extern_in(),
//         to: |db|
// Type::Value(db.item_id("ExampleEnum").into_value_type(db)).into_ref(),     }

//     cast_snapshot! {
//         name: enum_mut_in,
//         from: |db| db.item_id("ExampleEnum").into_value_type(db).extern_in(),
//         to: |db|
// Type::Value(db.item_id("ExampleEnum").into_value_type(db)).into_mut(),     }

//     cast_snapshot! {
//         name: enum_out,
//         from: |db|
// Type::Value(db.item_id("ExampleEnum").into_value_type(db)),         to: |db|
// db.item_id("ExampleEnum").into_value_type(db).extern_out(),     }
// }
