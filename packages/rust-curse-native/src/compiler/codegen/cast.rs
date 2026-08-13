//! A module for defining a graph of cast operations from one type to another.
//!
//! A cast graph consists of small independent cast rules
//! ([direct](DirectCastRule) or [indirect](IndirectCastRule)). These rules form
//! the edges of a directed graph, where a cast between two types is possible
//! if there exists a reflexive transitive path between them.
//!
//! For example, having 3 cast rules "A -> B", "B -> C", and "C -> D",
//! a cast from type "A" to "D" is possible by applying in order "A -> B", "B ->
//! C", and "C -> D".
//!
//! This module doesn't define the graph itself, but provides a
//! [`define_cast_graph`] macro for doing so. The module also doesn't assume a
//! target language, so it can be used for both Rust and JS code.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::Debug,
    hash::Hash,
};

use crate::compiler::{
    codegen::fragment::{FragmentOnce, Printer, fragment},
    db::Db,
};

/// Defines a cast rule that produces its target as an expression.
pub trait DirectCastRule<'db, From: 'db, To: 'db>: Sized + 'db {
    /// Returns the immediate source type from which this rule can produce `to`.
    fn predecessor(self, db: &'db dyn Db, to: To) -> Option<From>;

    /// Compiles an expression that converts `in_expr` from `from` to `to`.
    fn render<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        from: From,
        to: To,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment;
}

/// Defines a cast rule that controls the scope of the remaining cast pipeline.
pub trait IndirectCastRule<'db, From: 'db, To: 'db>: Sized + 'db {
    /// Returns the immediate source type from which this rule can produce `to`.
    fn predecessor(self, db: &'db dyn Db, to: To) -> Option<From>;

    /// Compiles a conversion and invokes `continue_with` with its result.
    fn render<'fragment, Expr, Continue>(
        self,
        db: &'db dyn Db,
        from: From,
        to: To,
        in_expr: Expr,
        continue_with: Continue,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
        Continue: CastContinuation<'db>;
}

/// Statically dispatches topology queries and compilation for a closed cast
/// graph.
pub trait CastGraph<'db> {
    /// Unified representation of every concrete type in the graph.
    type Type: Clone + Debug + Eq + Hash + 'db;

    /// Stable identifier of a cast rule.
    type Rule: Copy + Debug + Eq + 'static;

    /// All graph rules in deterministic search order.
    const RULES: &'static [Self::Rule];

    /// Returns the immediate predecessor from which `rule` can produce `to`.
    fn predecessor(db: &'db dyn Db, rule: Self::Rule, to: Self::Type) -> Option<Self::Type>;

    /// Compiles one instantiated cast step around its continuation.
    fn compile_step<'fragment, Expr, Continue>(
        db: &'db dyn Db,
        rule: Self::Rule,
        from: Self::Type,
        to: Self::Type,
        in_expr: Expr,
        continue_with: Continue,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
        Continue: CastContinuation<'db>;
}

/// Defines how code generation proceeds after a cast rule produces an
/// expression.
pub trait CastContinuation<'db>: Sized + 'db {
    /// Uses `out_expr` as the input to the next cast rule or the final
    /// operation.
    fn compile<'fragment, Expr>(self, out_expr: Expr) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment;
}

/// One concrete rule application in a cast path.
#[derive(Clone, Debug, PartialEq, Eq, salsa::Update)]
pub struct CastPathStep<Type, Rule> {
    pub rule: Rule,
    pub from: Type,
    pub to: Type,
}

/// Finds the concrete cast steps between two types in a graph.
#[doc(hidden)] // used only from the macro
pub fn find_cast_path<'db, Graph>(
    db: &'db dyn Db,
    from: Graph::Type,
    to: Graph::Type,
) -> Vec<CastPathStep<Graph::Type, Graph::Rule>>
where
    Graph: CastGraph<'db>,
{
    if from == to {
        return Vec::new();
    }

    let mut visited = HashSet::from([to.clone()]);
    let mut pending = VecDeque::from([to.clone()]);
    let mut steps_by_from = HashMap::new();

    while let Some(current) = pending.pop_front() {
        for &rule in Graph::RULES {
            let Some(predecessor) = Graph::predecessor(db, rule, current.clone()) else {
                continue;
            };

            if !visited.insert(predecessor.clone()) {
                continue;
            }

            steps_by_from.insert(
                predecessor.clone(),
                CastPathStep {
                    rule,
                    from: predecessor.clone(),
                    to: current.clone(),
                },
            );

            if predecessor == from {
                let mut steps = Vec::new();
                let mut current = from;

                while current != to {
                    let step = steps_by_from
                        .remove(&current)
                        .expect("every visited cast type except the target has a successor step");
                    current = step.to.clone();
                    steps.push(step);
                }

                return steps;
            }

            pending.push_back(predecessor);
        }
    }

    panic!("cannot cast from `{from:?}` to `{to:?}`: no cast path exists");
}

/// Compiles a previously planned cast into a target-specific continuation.
#[doc(hidden)] // used only from the macro
pub fn compile_cast_path<'db, Graph, Expr, Continue>(
    db: &'db dyn Db,
    in_expr: Expr,
    continue_with: Continue,
    path: &'db [CastPathStep<Graph::Type, Graph::Rule>],
) -> impl FragmentOnce + 'db
where
    Graph: CastGraph<'db> + 'db,
    Expr: FragmentOnce + 'db,
    Continue: CastContinuation<'db>,
{
    CastPathContinuation::<Graph, Continue> {
        db,
        remaining: path,
        continue_with,
    }
    .compile(in_expr)
}

/// Continues compilation through the remaining steps of one cast path.
struct CastPathContinuation<'db, Graph, Continue>
where
    Graph: CastGraph<'db>,
{
    db: &'db dyn Db,
    remaining: &'db [CastPathStep<Graph::Type, Graph::Rule>],
    continue_with: Continue,
}

impl<'db, Graph, Continue> CastContinuation<'db> for CastPathContinuation<'db, Graph, Continue>
where
    Graph: CastGraph<'db> + 'db,
    Continue: CastContinuation<'db>,
{
    fn compile<'fragment, Expr>(self, in_expr: Expr) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let Self {
            db,
            remaining,
            continue_with,
        } = self;

        fragment(move |printer| {
            let Some((step, remaining)) = remaining.split_first() else {
                continue_with.compile(in_expr).apply(printer);
                return;
            };

            let mut in_expr = Some(in_expr);
            let mut render_in_expr = |printer: &mut Printer| {
                in_expr
                    .take()
                    .expect("cast expression must only be applied once")
                    .apply(printer);
            };
            let in_expr = fragment(&mut render_in_expr as &mut dyn FnMut(&mut Printer));

            Graph::compile_step(
                db,
                step.rule,
                step.from.clone(),
                step.to.clone(),
                in_expr,
                Self {
                    db,
                    remaining,
                    continue_with,
                },
            )
            .apply(printer);
        })
    }
}

/// Defines a closed cast graph using typed predecessor and compile functions.
///
/// The macro generates:
/// - `CastType<'db>`: enum of all graph node families;
/// - `CastRuleId`: enum of all rules with their direct/indirect classification;
/// - `Graph`: static [`CastGraph`] dispatcher;
/// - `find_cast_path`: Salsa-tracked backwards path query;
/// - `compile_cast`: compiles a target-neutral cast request into a
///   continuation.
#[macro_export]
macro_rules! define_cast_graph {
    (
        $vis:vis $graph:ident < $db:lifetime > {
            types {
                $(
                    $type_variant:ident ( $type_ty:ty )
                ),* $(,)?
            }

            direct_rules {
                $(
                    $direct_rule_variant:ident {
                        rule: $direct_rule:expr,
                        from: $direct_from_variant:ident,
                        to: $direct_to_variant:ident $(,)?
                    }
                )* $(,)?
            }

            indirect_rules {
                $(
                    $indirect_rule_variant:ident {
                        rule: $indirect_rule:expr,
                        from: $indirect_from_variant:ident,
                        to: $indirect_to_variant:ident $(,)?
                    }
                )* $(,)?
            }
        }
    ) => {
        #[allow(non_snake_case, clippy::drop_non_drop)]
        $vis mod $graph {
            #[allow(unused_imports)]
            use super::*;

            /// A concrete node in the generated cast graph.
            #[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
            pub enum CastType<$db> {
                $(
                    $type_variant($type_ty),
                )*
                #[doc(hidden)]
                __Db(::std::marker::PhantomData<&$db ()>),
            }

            $(
                impl<$db> ::std::convert::From<$type_ty> for CastType<$db> {
                    fn from(value: $type_ty) -> Self {
                        Self::$type_variant(value)
                    }
                }
            )*

            /// Identifier of a registered cast rule.
            #[allow(clippy::enum_variant_names)]
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
            pub enum CastRuleId {
                $(
                    $direct_rule_variant,
                )*
                $(
                    $indirect_rule_variant,
                )*
            }

            impl CastRuleId {
                /// Returns whether this rule controls the scope of its continuation.
                #[allow(dead_code)]
                pub const fn is_indirect(self) -> bool {
                    match self {
                        $(
                            Self::$direct_rule_variant => false,
                        )*
                        $(
                            Self::$indirect_rule_variant => true,
                        )*
                    }
                }
            }

            /// Static dispatcher for the generated graph.
            #[derive(Copy, Clone)]
            pub struct Graph;

            impl<$db> $crate::compiler::codegen::cast::CastGraph<$db> for Graph {
                type Type = CastType<$db>;
                type Rule = CastRuleId;

                const RULES: &'static [Self::Rule] = &[
                    $(
                        Self::Rule::$direct_rule_variant,
                    )*
                    $(
                        Self::Rule::$indirect_rule_variant,
                    )*
                ];

                fn predecessor(
                    db: &$db dyn $crate::compiler::db::Db,
                    rule: Self::Rule,
                    to: Self::Type,
                ) -> ::std::option::Option<Self::Type> {
                    match (rule, to) {
                        $(
                            (
                                Self::Rule::$direct_rule_variant,
                                CastType::$direct_to_variant(to),
                            ) => $crate::compiler::codegen::cast::DirectCastRule::predecessor(
                                $direct_rule,
                                db,
                                to,
                            )
                            .map(CastType::$direct_from_variant),
                        )*
                        $(
                            (
                                Self::Rule::$indirect_rule_variant,
                                CastType::$indirect_to_variant(to),
                            ) => $crate::compiler::codegen::cast::IndirectCastRule::predecessor(
                                $indirect_rule,
                                db,
                                to,
                            )
                            .map(CastType::$indirect_from_variant),
                        )*
                        _ => None,
                    }
                }

                fn compile_step<'fragment, Expr, Continue>(
                    db: &$db dyn $crate::compiler::db::Db,
                    rule: Self::Rule,
                    from: Self::Type,
                    to: Self::Type,
                    in_expr: Expr,
                    continue_with: Continue,
                ) -> impl $crate::compiler::codegen::fragment::FragmentOnce + 'fragment
                where
                    $db: 'fragment,
                    Expr: $crate::compiler::codegen::fragment::FragmentOnce + 'fragment,
                    Continue: $crate::compiler::codegen::cast::CastContinuation<$db>,
                {
                    $crate::compiler::codegen::fragment::fragment(move |printer| {
                        let debug_from = from.clone();
                        let debug_to = to.clone();

                        match (rule, from, to) {
                            $(
                                (
                                    Self::Rule::$direct_rule_variant,
                                    CastType::$direct_from_variant(from),
                                    CastType::$direct_to_variant(to),
                                ) => {
                                    let out_expr =
                                        $crate::compiler::codegen::cast::DirectCastRule::render(
                                            $direct_rule,
                                            db,
                                            from,
                                            to,
                                            in_expr,
                                        );
                                    $crate::compiler::codegen::cast::CastContinuation::compile(
                                        continue_with,
                                        out_expr,
                                    )
                                    .apply(printer);
                                }
                            )*
                            $(
                                (
                                    Self::Rule::$indirect_rule_variant,
                                    CastType::$indirect_from_variant(from),
                                    CastType::$indirect_to_variant(to),
                                ) => {
                                    $crate::compiler::codegen::cast::IndirectCastRule::render(
                                        $indirect_rule,
                                        db,
                                        from,
                                        to,
                                        in_expr,
                                        continue_with,
                                    )
                                    .apply(printer);
                                }
                            )*
                            _ => panic!(
                                "cast path contains endpoints incompatible with rule `{rule:?}`: `{debug_from:?}` -> `{debug_to:?}`"
                            ),
                        }
                    })
                }
            }

            /// Finds a shortest path from `from` to `to`, using rule order to break equal-length ties.
            #[doc(hidden)]
            #[salsa::tracked(returns(ref))]
            pub fn find_cast_path<$db>(
                db: &$db dyn $crate::compiler::db::Db,
                from: self::CastType<$db>,
                to: self::CastType<$db>,
            ) -> ::std::vec::Vec<
                $crate::compiler::codegen::cast::CastPathStep<self::CastType<$db>, self::CastRuleId>,
            > {
                $crate::compiler::codegen::cast::find_cast_path::<self::Graph>(db, from, to)
            }

            /// Compiles one cast request into a target-specific continuation.
            #[doc(hidden)]
            #[allow(dead_code)]
            pub fn compile_cast<$db, From, To, Expr, Continue>(
                db: &$db dyn $crate::compiler::db::Db,
                from: From,
                to: To,
                in_expr: Expr,
                continue_with: Continue,
            ) -> impl $crate::compiler::codegen::fragment::FragmentOnce + $db
            where
                From: ::std::convert::Into<self::CastType<$db>> + $db,
                To: ::std::convert::Into<self::CastType<$db>> + $db,
                Expr: $crate::compiler::codegen::fragment::FragmentOnce + $db,
                Continue: $crate::compiler::codegen::cast::CastContinuation<$db>,
            {
                let path = self::find_cast_path(db, from.into(), to.into());

                $crate::compiler::codegen::cast::compile_cast_path::<self::Graph, _, _>(
                    db,
                    in_expr,
                    continue_with,
                    path,
                )
            }
        }
    };
}
