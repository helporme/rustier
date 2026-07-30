//! Rust-specific utilities for compiling casts used by generated bindings.

use crate::{
    codegen::{
        bindings::{
            bir::{BirBoundaryType, BirSourceType},
            enum_::{
                CastEnumBoundaryTypeToBorrowedSourceType, CastEnumBoundaryTypeToOwnedSourceType,
                CastEnumSourceTypeToBoundaryType,
            },
            primitive::{
                CastCharToU32BoundaryRule, CastF32ToF64BoundaryRule, CastF64ToF32Rule,
                CastF64ToSignedIntRule, CastF64ToUnsignedIntRule, CastJsAnyToBoolRule,
                CastJsAnyToF64Rule, CastJsAnyToStringRule, CastPrimitiveToSameBoundaryRule,
                CastStringToStrMutRule, CastStringToStrRefRule, CastU32ToCharRule,
            },
            struct_::{
                CastStructBoundaryTypeToBorrowedSourceType,
                CastStructBoundaryTypeToOwnedSourceType, CastStructSourceTypeToBoundaryType,
            },
        },
        cast::{CastContinuation, IndirectCastRule},
        db::Db,
        fragment::{FragmentOnce, Printer, fragment},
    },
    define_cast_graph, frag,
};

define_cast_graph! {
    pub BindingsCastGraph<'db> {
        types {
            BoundaryType(BirBoundaryType<'db>),
            SourceType(BirSourceType<'db>),
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
            StructSourceTypeToBoundaryType {
                rule: CastStructSourceTypeToBoundaryType,
                from: SourceType,
                to: BoundaryType,
            }
            EnumSourceTypeToBoundaryType {
                rule: CastEnumSourceTypeToBoundaryType,
                from: SourceType,
                to: BoundaryType,
            }
        }

        indirect_rules {
            OwnedTypeToBorrowedType {
                rule: CastOwnedTypeToBorrowedTypeRule,
                from: SourceType,
                to: SourceType,
            }
            StructBoundaryTypeToBorrowedSourceType {
                rule: CastStructBoundaryTypeToBorrowedSourceType,
                from: BoundaryType,
                to: SourceType,
            }
            EnumBoundaryTypeToBorrowedSourceType {
                rule: CastEnumBoundaryTypeToBorrowedSourceType,
                from: BoundaryType,
                to: SourceType,
            }
            StructBoundaryTypeToOwnedSourceType {
                rule: CastStructBoundaryTypeToOwnedSourceType,
                from: BoundaryType,
                to: SourceType,
            }
            EnumBoundaryTypeToOwnedSourceType {
                rule: CastEnumBoundaryTypeToOwnedSourceType,
                from: BoundaryType,
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

/// Compiles one cast into a named Rust binding before `inner_stmt`.
pub fn compile_cast_stmt<'db, From, To, Expr, Continue>(
    db: &'db dyn Db,
    cast: Cast<'db, From, To, Expr>,
    inner_stmt: Continue,
) -> impl FragmentOnce + 'db
where
    From: Into<BindingsCastGraph::CastType<'db>> + 'db,
    To: Into<BindingsCastGraph::CastType<'db>> + 'db,
    Expr: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    BindingsCastGraph::compile_cast(
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
    From: Into<BindingsCastGraph::CastType<'db>> + 'db,
    To: Into<BindingsCastGraph::CastType<'db>> + 'db,
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
    From: Into<BindingsCastGraph::CastType<'db>> + 'db,
    To: Into<BindingsCastGraph::CastType<'db>> + 'db,
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
    From: Into<BindingsCastGraph::CastType<'db>> + 'db,
    To: Into<BindingsCastGraph::CastType<'db>> + 'db,
    Expr: FragmentOnce + 'db,
{
    let from = from.into();
    let to = to.into();
    let uses_indirect_rule = BindingsCastGraph::find_cast_path(db, from.clone(), to.clone())
        .iter()
        .any(|step| {
            matches!(
                step.rule,
                BindingsCastGraph::CastRuleId::StructBoundaryTypeToBorrowedSourceType
                    | BindingsCastGraph::CastRuleId::EnumBoundaryTypeToBorrowedSourceType
                    | BindingsCastGraph::CastRuleId::StructBoundaryTypeToOwnedSourceType
                    | BindingsCastGraph::CastRuleId::EnumBoundaryTypeToOwnedSourceType
                    | BindingsCastGraph::CastRuleId::StringToStrMut
            )
        });

    fragment(move |printer| {
        if uses_indirect_rule {
            frag!({{
                let mut __cast_expr_result = std::mem::MaybeUninit::uninit();@
                {BindingsCastGraph::compile_cast(
                    db,
                    from,
                    to,
                    expr,
                    WriteCastOutput {
                        out_var_name: "__cast_expr_result",
                    },
                )}@
                unsafe~{{
                    __cast_expr_result.assume_init()
                }}
            }})
            .apply(printer);
        } else {
            BindingsCastGraph::compile_cast(db, from, to, expr, ReturnCastOutput).apply(printer);
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
    From: Into<BindingsCastGraph::CastType<'db>> + 'db,
    To: Into<BindingsCastGraph::CastType<'db>> + 'db,
    Expr: FragmentOnce + 'db,
    Inner: FragmentOnce + 'db,
{
    CompileCastsExpr {
        db,
        casts: casts.into_iter(),
        inner_expr,
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
    From: Into<BindingsCastGraph::CastType<'db>> + 'db,
    To: Into<BindingsCastGraph::CastType<'db>> + 'db,
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
        let uses_indirect_rule =
            BindingsCastGraph::find_cast_path(self.db, from.clone(), to.clone())
                .iter()
                .any(|step| {
                    matches!(
                        step.rule,
                        BindingsCastGraph::CastRuleId::StructBoundaryTypeToBorrowedSourceType
                            | BindingsCastGraph::CastRuleId::EnumBoundaryTypeToBorrowedSourceType
                            | BindingsCastGraph::CastRuleId::StructBoundaryTypeToOwnedSourceType
                            | BindingsCastGraph::CastRuleId::EnumBoundaryTypeToOwnedSourceType
                            | BindingsCastGraph::CastRuleId::StringToStrMut
                    )
                });
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
                let mut __cast_expr_result = std::mem::MaybeUninit::uninit();@
                {compile_cast_stmt(
                    self.db,
                    cast,
                    frag!(__cast_expr_result.write({remaining});),
                )}@
                unsafe~{{
                    __cast_expr_result.assume_init()
                }}
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
        frag!({out_var_name}.write({out_expr});)
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

impl<'db> IndirectCastRule<'db, BirSourceType<'db>, BirSourceType<'db>>
    for CastOwnedTypeToBorrowedTypeRule
{
    fn predecessor(self, _db: &'db dyn Db, to: BirSourceType<'db>) -> Option<BirSourceType<'db>> {
        match to {
            BirSourceType::Ref { inner, .. } if inner.allows_generic_borrowing_cast() => {
                Some(*inner)
            }
            _ => None,
        }
    }

    fn compile<'fragment, Expr, Continue>(
        self,
        _db: &'db dyn Db,
        _from: BirSourceType<'db>,
        to: BirSourceType<'db>,
        in_expr: Expr,
        continue_with: Continue,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
        Continue: CastContinuation<'db>,
    {
        let BirSourceType::Ref { is_mutable, .. } = to else {
            unreachable!()
        };

        let borrowing = if is_mutable {
            frag!(&mut value)
        } else {
            frag!(&value)
        };
        let continue_with = continue_with.compile(borrowing);

        frag!(
            let mut value = {in_expr};
            {continue_with}
        )
    }
}

#[cfg(test)]
pub mod test {
    use crate::{
        codegen::{cast::CastContinuation, fragment::FragmentOnce},
        frag,
    };

    pub struct Consume;

    impl<'db> CastContinuation<'db> for Consume {
        fn compile<'fragment, Expr>(self, out_expr: Expr) -> impl FragmentOnce + 'fragment
        where
            'db: 'fragment,
            Expr: FragmentOnce + 'fragment,
        {
            frag!(consume({out_expr});)
        }
    }
}
