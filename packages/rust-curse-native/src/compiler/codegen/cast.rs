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
    fn compile<'fragment, Expr>(
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
    fn compile<'fragment, Expr, Continue>(
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
                                        $crate::compiler::codegen::cast::DirectCastRule::compile(
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
                                    $crate::compiler::codegen::cast::IndirectCastRule::compile(
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

#[cfg(test)]
mod tests {
    use crate::{
        compiler::{
            codegen::cast::{CastContinuation, CastPathStep, DirectCastRule, IndirectCastRule},
            codegen::fragment::{FragmentOnce, FragmentPrint},
            db::{Db, DbImpl},
            test::assert_snap,
        },
        frag,
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum CastKind {
        Direct,
        Indirect,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
    pub enum HappyScenario {
        OneDirect,
        OneIndirect,
        DirectDirect,
        DirectIndirect,
        IndirectDirect,
        IndirectIndirect,
        DirectIndirectDirect,
        IndirectDirectIndirect,
    }

    impl HappyScenario {
        fn cast_kinds(self) -> &'static [CastKind] {
            use CastKind::{Direct, Indirect};

            match self {
                Self::OneDirect => &[Direct],
                Self::OneIndirect => &[Indirect],
                Self::DirectDirect => &[Direct, Direct],
                Self::DirectIndirect => &[Direct, Indirect],
                Self::IndirectDirect => &[Indirect, Direct],
                Self::IndirectIndirect => &[Indirect, Indirect],
                Self::DirectIndirectDirect => &[Direct, Indirect, Direct],
                Self::IndirectDirectIndirect => &[Indirect, Direct, Indirect],
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
    pub struct HappyType {
        scenario: HappyScenario,
        step: u8,
    }

    impl HappyType {
        fn start(scenario: HappyScenario) -> Self {
            Self { scenario, step: 0 }
        }

        fn end(scenario: HappyScenario) -> Self {
            Self {
                scenario,
                step: scenario.cast_kinds().len() as u8,
            }
        }
    }

    fn predecessor(to: HappyType, kind: CastKind) -> Option<HappyType> {
        let previous_step = to.step.checked_sub(1)?;
        (to.scenario.cast_kinds()[previous_step as usize] == kind).then_some(HappyType {
            scenario: to.scenario,
            step: previous_step,
        })
    }

    struct DirectRule;

    impl<'db> DirectCastRule<'db, HappyType, HappyType> for DirectRule {
        fn predecessor(self, _db: &'db dyn Db, to: HappyType) -> Option<HappyType> {
            predecessor(to, CastKind::Direct)
        }

        fn compile<'fragment, Expr>(
            self,
            _db: &'db dyn Db,
            _from: HappyType,
            _to: HappyType,
            in_expr: Expr,
        ) -> impl FragmentOnce + 'fragment
        where
            'db: 'fragment,
            Expr: FragmentOnce + 'fragment,
        {
            frag!(direct({ in_expr }))
        }
    }

    struct IndirectRule;

    impl<'db> IndirectCastRule<'db, HappyType, HappyType> for IndirectRule {
        fn predecessor(self, _db: &'db dyn Db, to: HappyType) -> Option<HappyType> {
            predecessor(to, CastKind::Indirect)
        }

        fn compile<'fragment, Expr, Continue>(
            self,
            _db: &'db dyn Db,
            _from: HappyType,
            _to: HappyType,
            in_expr: Expr,
            continue_with: Continue,
        ) -> impl FragmentOnce + 'fragment
        where
            'db: 'fragment,
            Expr: FragmentOnce + 'fragment,
            Continue: CastContinuation<'db>,
        {
            continue_with.compile(frag!(indirect({ in_expr })))
        }
    }

    struct Sink;

    impl<'db> CastContinuation<'db> for Sink {
        fn compile<'fragment, Expr>(self, out_expr: Expr) -> impl FragmentOnce + 'fragment
        where
            'db: 'fragment,
            Expr: FragmentOnce + 'fragment,
        {
            frag!(sink({ out_expr }))
        }
    }

    define_cast_graph! {
        pub HappyCastGraph<'db> {
            types {
                HappyType(super::HappyType),
            }

            direct_rules {
                Direct {
                    rule: DirectRule,
                    from: HappyType,
                    to: HappyType,
                }
            }

            indirect_rules {
                Indirect {
                    rule: IndirectRule,
                    from: HappyType,
                    to: HappyType,
                }
            }
        }
    }

    fn assert_happy_path(
        path: &[CastPathStep<HappyCastGraph::CastType<'_>, HappyCastGraph::CastRuleId>],
        scenario: HappyScenario,
        rules: &[HappyCastGraph::CastRuleId],
    ) {
        assert_eq!(path.len(), rules.len());

        for (index, (step, &rule)) in path.iter().zip(rules).enumerate() {
            assert_eq!(
                step,
                &CastPathStep {
                    rule,
                    from: HappyType {
                        scenario,
                        step: index as u8,
                    }
                    .into(),
                    to: HappyType {
                        scenario,
                        step: index as u8 + 1,
                    }
                    .into(),
                },
            );
        }
    }

    macro_rules! happy_path_test {
        ($name:ident, $scenario:ident, [$($rule:ident),+ $(,)?]) => {
            #[test]
            fn $name() {
                let db = DbImpl::default();
                let from = HappyType::start(HappyScenario::$scenario);
                let to = HappyType::end(HappyScenario::$scenario);
                let path = HappyCastGraph::find_cast_path(&db, from.into(), to.into());

                assert_happy_path(
                    path,
                    HappyScenario::$scenario,
                    &[$(HappyCastGraph::CastRuleId::$rule),+],
                );
                assert_eq!(
                    path.iter()
                        .map(|step| step.rule.is_indirect())
                        .collect::<Vec<_>>(),
                    HappyScenario::$scenario
                        .cast_kinds()
                        .iter()
                        .map(|kind| *kind == CastKind::Indirect)
                        .collect::<Vec<_>>(),
                );

                assert_snap!({
                    HappyCastGraph::compile_cast(
                        &db,
                        from,
                        to,
                        frag!(source()),
                        Sink,
                    )
                    .print()
                });
            }
        };
    }

    happy_path_test!(one_direct_step, OneDirect, [Direct]);
    happy_path_test!(one_indirect_step, OneIndirect, [Indirect]);
    happy_path_test!(direct_direct_path, DirectDirect, [Direct, Direct]);
    happy_path_test!(direct_indirect_path, DirectIndirect, [Direct, Indirect]);
    happy_path_test!(indirect_direct_path, IndirectDirect, [Indirect, Direct]);
    happy_path_test!(
        indirect_indirect_path,
        IndirectIndirect,
        [Indirect, Indirect]
    );
    happy_path_test!(
        direct_indirect_direct_path,
        DirectIndirectDirect,
        [Direct, Indirect, Direct]
    );
    happy_path_test!(
        indirect_direct_indirect_path,
        IndirectDirectIndirect,
        [Indirect, Direct, Indirect]
    );

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
    pub enum EdgeType {
        CycleA,
        CycleB,
        CycleC,
        DisconnectedFrom,
        DisconnectedTo,
        Standalone,
        ShortestFrom,
        ShortestIntermediate,
        ShortestTo,
        EqualFrom,
        EqualEarlierIntermediate,
        EqualLaterIntermediate,
        EqualTo,
    }

    struct EdgeRule {
        from: EdgeType,
        to: EdgeType,
    }

    impl<'db> DirectCastRule<'db, EdgeType, EdgeType> for EdgeRule {
        fn predecessor(self, _db: &'db dyn Db, to: EdgeType) -> Option<EdgeType> {
            (to == self.to).then_some(self.from)
        }

        fn compile<'fragment, Expr>(
            self,
            _db: &'db dyn Db,
            _from: EdgeType,
            _to: EdgeType,
            in_expr: Expr,
        ) -> impl FragmentOnce + 'fragment
        where
            'db: 'fragment,
            Expr: FragmentOnce + 'fragment,
        {
            in_expr
        }
    }

    define_cast_graph! {
        pub EdgeCastGraph<'db> {
            types {
                EdgeType(super::EdgeType),
            }

            direct_rules {
                LongerPathFirstStep {
                    rule: EdgeRule {
                        from: EdgeType::ShortestFrom,
                        to: EdgeType::ShortestIntermediate,
                    },
                    from: EdgeType,
                    to: EdgeType,
                }

                LongerPathSecondStep {
                    rule: EdgeRule {
                        from: EdgeType::ShortestIntermediate,
                        to: EdgeType::ShortestTo,
                    },
                    from: EdgeType,
                    to: EdgeType,
                }

                DirectShortestPath {
                    rule: EdgeRule {
                        from: EdgeType::ShortestFrom,
                        to: EdgeType::ShortestTo,
                    },
                    from: EdgeType,
                    to: EdgeType,
                }

                EqualEarlierFirstStep {
                    rule: EdgeRule {
                        from: EdgeType::EqualFrom,
                        to: EdgeType::EqualEarlierIntermediate,
                    },
                    from: EdgeType,
                    to: EdgeType,
                }

                EqualEarlierSecondStep {
                    rule: EdgeRule {
                        from: EdgeType::EqualEarlierIntermediate,
                        to: EdgeType::EqualTo,
                    },
                    from: EdgeType,
                    to: EdgeType,
                }

                EqualLaterFirstStep {
                    rule: EdgeRule {
                        from: EdgeType::EqualFrom,
                        to: EdgeType::EqualLaterIntermediate,
                    },
                    from: EdgeType,
                    to: EdgeType,
                }

                EqualLaterSecondStep {
                    rule: EdgeRule {
                        from: EdgeType::EqualLaterIntermediate,
                        to: EdgeType::EqualTo,
                    },
                    from: EdgeType,
                    to: EdgeType,
                }

                CycleAToB {
                    rule: EdgeRule {
                        from: EdgeType::CycleA,
                        to: EdgeType::CycleB,
                    },
                    from: EdgeType,
                    to: EdgeType,
                }

                CycleBToC {
                    rule: EdgeRule {
                        from: EdgeType::CycleB,
                        to: EdgeType::CycleC,
                    },
                    from: EdgeType,
                    to: EdgeType,
                }

                CycleCToA {
                    rule: EdgeRule {
                        from: EdgeType::CycleC,
                        to: EdgeType::CycleA,
                    },
                    from: EdgeType,
                    to: EdgeType,
                }
            }

            indirect_rules {}
        }
    }

    #[test]
    fn direct_path_wins_over_earlier_longer_path() {
        let db = DbImpl::default();
        let path = EdgeCastGraph::find_cast_path(
            &db,
            EdgeType::ShortestFrom.into(),
            EdgeType::ShortestTo.into(),
        );

        assert_eq!(
            path.as_slice(),
            &[CastPathStep {
                rule: EdgeCastGraph::CastRuleId::DirectShortestPath,
                from: EdgeType::ShortestFrom.into(),
                to: EdgeType::ShortestTo.into(),
            }],
        );
    }

    #[test]
    fn rule_order_breaks_equal_length_tie() {
        let db = DbImpl::default();
        let path = EdgeCastGraph::find_cast_path(
            &db,
            EdgeType::EqualFrom.into(),
            EdgeType::EqualTo.into(),
        );

        assert_eq!(
            path.as_slice(),
            &[
                CastPathStep {
                    rule: EdgeCastGraph::CastRuleId::EqualEarlierFirstStep,
                    from: EdgeType::EqualFrom.into(),
                    to: EdgeType::EqualEarlierIntermediate.into(),
                },
                CastPathStep {
                    rule: EdgeCastGraph::CastRuleId::EqualEarlierSecondStep,
                    from: EdgeType::EqualEarlierIntermediate.into(),
                    to: EdgeType::EqualTo.into(),
                },
            ],
        );
    }

    #[test]
    #[should_panic(expected = "no cast path exists")]
    fn no_path_between_types() {
        let db = DbImpl::default();
        EdgeCastGraph::find_cast_path(
            &db,
            EdgeType::DisconnectedFrom.into(),
            EdgeType::DisconnectedTo.into(),
        );
    }

    #[test]
    fn same_cyclic_type_has_empty_path() {
        let db = DbImpl::default();
        let path =
            EdgeCastGraph::find_cast_path(&db, EdgeType::CycleA.into(), EdgeType::CycleA.into());

        assert!(path.is_empty());
    }

    #[test]
    fn finds_two_step_path_in_cycle() {
        let db = DbImpl::default();
        let path =
            EdgeCastGraph::find_cast_path(&db, EdgeType::CycleA.into(), EdgeType::CycleC.into());

        assert_eq!(
            path.as_slice(),
            &[
                CastPathStep {
                    rule: EdgeCastGraph::CastRuleId::CycleAToB,
                    from: EdgeType::CycleA.into(),
                    to: EdgeType::CycleB.into(),
                },
                CastPathStep {
                    rule: EdgeCastGraph::CastRuleId::CycleBToC,
                    from: EdgeType::CycleB.into(),
                    to: EdgeType::CycleC.into(),
                },
            ],
        );
    }

    #[test]
    fn path_to_itself_is_empty() {
        let db = DbImpl::default();
        let path = EdgeCastGraph::find_cast_path(
            &db,
            EdgeType::Standalone.into(),
            EdgeType::Standalone.into(),
        );

        assert!(path.is_empty());
    }
}
