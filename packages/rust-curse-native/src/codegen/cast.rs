//! A module for defining a graph of "cast" operations from one type to another.
//! Using the cast graph, you can cast an expression to another type in case
//! there is a path between the input expression type and the target type.
//!
//! This module doesn't define the graph itself, but provides a
//! [`define_cast_graph`] macro for doing so.
//!
//! "Cast" doesn't neccessary mean the number casting. It's any operation that
//! converts one type to another.

use std::{collections::HashSet, fmt::Debug, hash::Hash};

use crate::{
    codegen::{
        db::Db,
        fragment::{FragmentOnce, fragment},
    },
    frag,
};

/// Defines a cast rule that produces its target as an expression.
pub trait DirectCastRule<'db, From: 'db, To: 'db>: Sized + 'db {
    /// Returns the immediate source type from which this rule can produce `to`.
    fn predecessor(self, db: &'db dyn Db, to: To) -> Option<From>;

    /// Compiles an expression that converts `in_expr` from `from` to `to`.
    fn compile(
        self,
        db: &'db dyn Db,
        from: From,
        to: To,
        in_expr: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db;
}

/// Defines a cast rule that controls the scope of the remaining cast pipeline.
pub trait IndirectCastRule<'db, From: 'db, To: 'db>: Sized + 'db {
    /// Returns the immediate source type from which this rule can produce `to`.
    fn predecessor(self, db: &'db dyn Db, to: To) -> Option<From>;

    /// Compiles a conversion and invokes `continue_with` with its result bound
    /// as `value`.
    fn compile(
        self,
        db: &'db dyn Db,
        from: From,
        to: To,
        in_expr: impl FragmentOnce + 'db,
        continue_with: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db;
}

impl<'db, From: 'db, To: 'db, Rule> IndirectCastRule<'db, From, To> for Rule
where
    Rule: DirectCastRule<'db, From, To>,
{
    fn predecessor(self, db: &'db dyn Db, to: To) -> Option<From> {
        DirectCastRule::predecessor(self, db, to)
    }

    fn compile(
        self,
        db: &'db dyn Db,
        from: From,
        to: To,
        in_expr: impl FragmentOnce + 'db,
        continue_with: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
        let cast_expr = DirectCastRule::compile(self, db, from, to, in_expr);

        frag!(
            let value = ~{cast_expr};@
            {continue_with}
        )
    }
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
    fn compile_step<Expr, Continue>(
        db: &'db dyn Db,
        rule: Self::Rule,
        from: Self::Type,
        to: Self::Type,
        in_expr: Expr,
        continue_with: Continue,
    ) -> impl FragmentOnce + 'db
    where
        Expr: FragmentOnce + 'db,
        Continue: FragmentOnce + 'db;
}

/// One concrete rule application in a cast path.
#[derive(Clone, Debug, PartialEq, Eq, salsa::Update)]
pub struct CastPathStep<Type, Rule> {
    pub rule: Rule,
    pub from: Type,
    pub to: Type,
}

/// Finds the concrete cast steps between two types in a graph.
pub fn find_cast_path<'db, Graph>(
    db: &'db dyn Db,
    from: Graph::Type,
    to: Graph::Type,
) -> Vec<CastPathStep<Graph::Type, Graph::Rule>>
where
    Graph: CastGraph<'db>,
{
    let mut visited = HashSet::new();
    let mut steps = Vec::new();

    assert!(
        find_cast_path_from::<Graph>(db, from.clone(), to.clone(), &mut visited, &mut steps),
        "cannot cast from `{from:?}` to `{to:?}`: no cast path exists",
    );

    steps
}

fn find_cast_path_from<'db, Graph>(
    db: &'db dyn Db,
    from: Graph::Type,
    current: Graph::Type,
    visited: &mut HashSet<Graph::Type>,
    steps: &mut Vec<CastPathStep<Graph::Type, Graph::Rule>>,
) -> bool
where
    Graph: CastGraph<'db>,
{
    if current == from {
        return true;
    }

    if !visited.insert(current.clone()) {
        return false;
    }

    for &rule in Graph::RULES {
        let Some(predecessor) = Graph::predecessor(db, rule, current.clone()) else {
            continue;
        };

        if find_cast_path_from::<Graph>(db, from.clone(), predecessor.clone(), visited, steps) {
            steps.push(CastPathStep {
                rule,
                from: predecessor,
                to: current,
            });
            return true;
        }
    }

    false
}

/// Compiles a previously planned cast and exposes its result as `value`.
pub fn compile_cast_path<'db, Graph, Expr, Access>(
    db: &'db dyn Db,
    in_expr: Expr,
    access: Access,
    path: &'db [CastPathStep<Graph::Type, Graph::Rule>],
) -> impl FragmentOnce + 'db
where
    Graph: CastGraph<'db> + 'db,
    Expr: FragmentOnce + 'db,
    Access: FragmentOnce + 'db,
{
    let steps = compile_cast_steps::<Graph, _>(db, path, access);

    frag!(
        let value = ~{in_expr};@
        {steps}
    )
}

fn compile_cast_steps<'db, Graph, Access>(
    db: &'db dyn Db,
    remaining: &'db [CastPathStep<Graph::Type, Graph::Rule>],
    access: Access,
) -> impl FragmentOnce + 'db
where
    Graph: CastGraph<'db> + 'db,
    Access: FragmentOnce + 'db,
{
    fragment(move |printer| {
        let Some((step, remaining)) = remaining.split_first() else {
            access.apply(printer);
            return;
        };

        let continue_with = compile_cast_steps::<Graph, _>(db, remaining, access);
        Graph::compile_step(
            db,
            step.rule,
            step.from.clone(),
            step.to.clone(),
            "value",
            continue_with,
        )
        .apply(printer);
    })
}

/// A request to compile a cast from `from` to `to` and expose the result as
/// `value` to `access`.
pub struct Cast<'db, From, To, Expr, Access> {
    pub db: &'db dyn Db,
    pub from: From,
    pub to: To,
    pub in_expr: Expr,
    pub access: Access,
}

/// Defines a closed cast graph using typed predecessor and compile functions.
///
/// The macro generates:
/// - `CastType<'db>`: enum of all graph node families;
/// - `CastRuleId`: enum of all rules;
/// - `Graph`: static [`CastGraph`] dispatcher;
/// - `find_cast_path`: Salsa-tracked backwards path query;
/// - `compile_cast`: convenience function compiling one cast request.
#[macro_export]
macro_rules! define_cast_graph {
    (
        $vis:vis $graph:ident < $db:lifetime > {
            types {
                $(
                    $type_variant:ident ( $type_ty:ty )
                ),* $(,)?
            }

            rules {
                $(
                    $rule_variant:ident {
                        rule: $rule:expr,
                        from: $from_variant:ident,
                        to: $to_variant:ident $(,)?
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
                    $rule_variant,
                )*
            }

            /// Static dispatcher for the generated graph.
            pub struct Graph;

            impl<$db> $crate::codegen::cast::CastGraph<$db> for Graph {
                type Type = CastType<$db>;
                type Rule = CastRuleId;

                const RULES: &'static [Self::Rule] = &[
                    $(
                        Self::Rule::$rule_variant,
                    )*
                ];

                fn predecessor(
                    db: &$db dyn $crate::codegen::db::Db,
                    rule: Self::Rule,
                    to: Self::Type,
                ) -> ::std::option::Option<Self::Type> {
                    match (rule, to) {
                        $(
                            (
                                Self::Rule::$rule_variant,
                                CastType::$to_variant(to),
                            ) => $crate::codegen::cast::IndirectCastRule::predecessor(
                                $rule,
                                db,
                                to,
                            )
                            .map(CastType::$from_variant),
                        )*
                        _ => None,
                    }
                }

                fn compile_step<Expr, Continue>(
                    db: &$db dyn $crate::codegen::db::Db,
                    rule: Self::Rule,
                    from: Self::Type,
                    to: Self::Type,
                    in_expr: Expr,
                    continue_with: Continue,
                ) -> impl $crate::codegen::fragment::FragmentOnce + $db
                where
                    Expr: $crate::codegen::fragment::FragmentOnce + $db,
                    Continue: $crate::codegen::fragment::FragmentOnce + $db,
                {
                    $crate::codegen::fragment::fragment(move |p| {
                        let debug_from = from.clone();
                        let debug_to = to.clone();

                        match (rule, from, to) {
                            $(
                                (
                                    Self::Rule::$rule_variant,
                                    CastType::$from_variant(from),
                                    CastType::$to_variant(to),
                                ) => {
                                    let compiled =
                                        $crate::codegen::cast::IndirectCastRule::compile(
                                            $rule,
                                            db,
                                            from,
                                            to,
                                            in_expr,
                                            continue_with,
                                        );
                                    $crate::codegen::fragment::FragmentOnce::apply(compiled, p);
                                }
                            )*
                            _ => panic!(
                                "cast path contains endpoints incompatible with rule `{rule:?}`: `{debug_from:?}` -> `{debug_to:?}`"
                            ),
                        }
                    })
                }
            }

            /// Finds the unique path from `from` to `to`.
            #[salsa::tracked(returns(ref))]
            pub fn find_cast_path<$db>(
                db: &$db dyn $crate::codegen::db::Db,
                from: self::CastType<$db>,
                to: self::CastType<$db>,
            ) -> ::std::vec::Vec<
                $crate::codegen::cast::CastPathStep<self::CastType<$db>, self::CastRuleId>,
            > {
                $crate::codegen::cast::find_cast_path::<self::Graph>(db, from, to)
            }

            /// Compiles a cast request and exposes the converted value as `value`.
            #[allow(dead_code)]
            pub fn compile_cast<$db, From, To, Expr, Access>(
                cast: $crate::codegen::cast::Cast<$db, From, To, Expr, Access>,
            ) -> impl $crate::codegen::fragment::FragmentOnce + $db
            where
                From: ::std::convert::Into<self::CastType<$db>> + $db,
                To: ::std::convert::Into<self::CastType<$db>> + $db,
                Expr: $crate::codegen::fragment::FragmentOnce + $db,
                Access: $crate::codegen::fragment::FragmentOnce + $db,
            {
                let db = cast.db;
                let from = cast.from.into();
                let to = cast.to.into();
                let path = self::find_cast_path(db, from, to);

                $crate::codegen::cast::compile_cast_path::<self::Graph, _, _>(
                    db,
                    cast.in_expr,
                    cast.access,
                    path,
                )
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::{
        codegen::{
            cast::{Cast, CastPathStep, DirectCastRule, IndirectCastRule},
            db::{Db, DbImpl},
            fragment::{FragmentOnce, FragmentPrint},
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

        fn compile(
            self,
            _db: &'db dyn Db,
            _from: HappyType,
            _to: HappyType,
            in_expr: impl FragmentOnce + 'db,
        ) -> impl FragmentOnce + 'db {
            frag!(direct({ in_expr }))
        }
    }

    struct IndirectRule;

    impl<'db> IndirectCastRule<'db, HappyType, HappyType> for IndirectRule {
        fn predecessor(self, _db: &'db dyn Db, to: HappyType) -> Option<HappyType> {
            predecessor(to, CastKind::Indirect)
        }

        fn compile(
            self,
            _db: &'db dyn Db,
            _from: HappyType,
            _to: HappyType,
            in_expr: impl FragmentOnce + 'db,
            continue_with: impl FragmentOnce + 'db,
        ) -> impl FragmentOnce + 'db {
            frag!(
                {{
                    let value = indirect({in_expr});@
                    {continue_with}@
                }}
            )
        }
    }

    define_cast_graph! {
        pub HappyCastGraph<'db> {
            types {
                HappyType(super::HappyType),
            }

            rules {
                Direct {
                    rule: DirectRule,
                    from: HappyType,
                    to: HappyType,
                }

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

                assert_snap!({
                    HappyCastGraph::compile_cast(Cast {
                        db: &db,
                        from,
                        to,
                        in_expr: frag!(source()),
                        access: "consume(value);",
                    })
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
    }

    struct EdgeRule {
        from: EdgeType,
        to: EdgeType,
    }

    impl<'db> DirectCastRule<'db, EdgeType, EdgeType> for EdgeRule {
        fn predecessor(self, _db: &'db dyn Db, to: EdgeType) -> Option<EdgeType> {
            (to == self.to).then_some(self.from)
        }

        fn compile(
            self,
            _db: &'db dyn Db,
            _from: EdgeType,
            _to: EdgeType,
            in_expr: impl FragmentOnce + 'db,
        ) -> impl FragmentOnce + 'db {
            frag!({ in_expr })
        }
    }

    define_cast_graph! {
        pub EdgeCastGraph<'db> {
            types {
                EdgeType(super::EdgeType),
            }

            rules {
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
        }
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
