use ra_ap_hir::{
    AsAssocItem as _, FindPathConfig, HirDisplay as _, ModuleDef, PathResolution,
    PredicateEvaluationStatus, PrefixKind, Semantics, db::HirDatabase,
};
use ra_ap_syntax::{
    AstNode as _, SyntaxNode, TextRange,
    ast::{self, HasGenericParams as _, make},
    syntax_editor::SyntaxEditor,
};

use crate::compiler::{
    db::Db,
    ir::rust::{
        Callable, Crate, FunctionInherent, FunctionQualified, GenArg, GenericParam, ImplQualified,
        ItemPath, Module, Param, PathSegment, Type, TypeKind, TypeOrigin,
    },
};

#[salsa::tracked]
impl<'db> FunctionInherent<'db> {
    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_as_callable(
        self,
        db: &'db dyn Db,
    ) -> Option<Callable<'db>> {
        let impl_ = self.impl_(db);
        let substituted_args = impl_
            .generic_args(db)
            .iter()
            .filter(|(_, arg)| {
                let GenArg::Type(type_) = arg;
                let hir = type_.hir(db);
                !hir.contains_unknown() && hir.generic_params(db).is_empty()
            })
            .count();

        if !self.declaration(db).generic_params(db).is_empty()
            || substituted_args != impl_.declaration(db).generic_params(db).len()
        {
            return None;
        }

        let callable = Callable::new(
            db,
            FunctionQualified::Inherent(self),
            Vec::<GenArg<'db>>::new(),
        );

        evaluate_where_clause(db, callable)?.then_some(callable)
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_path(self, db: &'db dyn Db) -> ItemPath<'db> {
        let target = self.impl_(db).target(db);
        let path = match target.kind(db) {
            TypeKind::Struct(type_) => type_.path(db),
            TypeKind::Enum(type_) => type_.path(db),
            _ => {
                let hir = target.hir(db);
                if let Some(adt) = hir.as_adt() {
                    let module = adt.module(db);
                    let name = adt
                        .name(db)
                        .display(db, module.krate(db).edition(db))
                        .to_string();
                    Module::new(db, module)
                        .path(db)
                        .append(db, PathSegment::plain(db, name))
                } else {
                    let crate_ = self.impl_(db).declaration(db).hir(db).module(db).krate(db);
                    ItemPath::from_segment(
                        db,
                        Crate::new(db, crate_),
                        PathSegment::plain(db, target.display(db).to_owned()),
                    )
                }
            }
        };
        path.append(db, self.declaration(db).segment(db))
    }
}

#[salsa::tracked]
impl<'db> Callable<'db> {
    pub(in crate::compiler::ir::rust) fn needs_substitution(self, db: &'db dyn Db) -> bool {
        !self.declaration(db).generic_params(db).is_empty()
            || match self.function(db) {
                FunctionQualified::Free(_) => false,
                FunctionQualified::Trait(_) => true,
                FunctionQualified::Inherent(function) => !function
                    .impl_(db)
                    .declaration(db)
                    .generic_params(db)
                    .is_empty(),
            }
    }

    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::rust) fn lower_params(self, db: &'db dyn Db) -> Vec<Param<'db>> {
        if !self.needs_substitution(db) {
            return self.declaration(db).params(db).clone();
        }

        self.declaration(db)
            .params(db)
            .iter()
            .map(|param| {
                Param::new(
                    db,
                    param.index(db),
                    param.name(db).clone(),
                    Type::new(
                        db,
                        TypeOrigin::CallableParameter {
                            callable: self,
                            index: param.index(db),
                        },
                    ),
                    param.span(db).clone(),
                )
            })
            .collect()
    }

    #[salsa::tracked]
    pub(in crate::compiler::ir::rust) fn lower_ret_type(self, db: &'db dyn Db) -> Type<'db> {
        if !self.needs_substitution(db) {
            return self.declaration(db).ret_type(db);
        }
        Type::new(db, TypeOrigin::CallableReturn(self))
    }

    pub(super) fn instantiate(
        self,
        db: &'db dyn Db,
        type_: ra_ap_hir::Type<'db>,
    ) -> ra_ap_hir::Type<'db> {
        let mut args = Vec::new();
        if let Some(impl_) = self.function(db).impl_(db) {
            args.extend(impl_.function_type_args(db));
        }

        args.extend(self.generic_args(db).iter().map(|arg| {
            let GenArg::Type(type_) = arg;
            type_.hir(db)
        }));

        super::instantiate_type(db, type_, args, self.context(db))
    }

    pub(super) fn context(self, db: &'db dyn Db) -> ra_ap_hir::GenericDef {
        match self.function(db) {
            FunctionQualified::Free(function) => {
                ra_ap_hir::GenericDef::Function(function.declaration(db).hir(db))
            }
            function => function
                .impl_(db)
                .expect("associated function")
                .target(db)
                .origin(db)
                .context(db),
        }
    }
}

impl<'db> ImplQualified<'db> {
    pub(super) fn function_type_args(self, db: &'db dyn Db) -> Vec<ra_ap_hir::Type<'db>> {
        match self {
            Self::Inherent(_) => self.partial_type_args(db),
            Self::Trait(impl_) => std::iter::once(impl_.target(db).hir(db))
                .chain(impl_.trait_args(db).iter().map(|arg| {
                    let GenArg::Type(type_) = arg;

                    type_.hir(db)
                }))
                .collect(),
        }
    }

    /// Preserve identity arguments for bindings not determined by the target.
    pub(super) fn partial_type_args(self, db: &'db dyn Db) -> Vec<ra_ap_hir::Type<'db>> {
        self.declaration(db)
            .generic_params(db)
            .iter()
            .map(|param| {
                let GenericParam::Type(type_param) = param;
                self.generic_args(db)
                    .iter()
                    .find(|(binding, _)| binding == param)
                    .map(|(_, arg)| {
                        let GenArg::Type(type_) = arg;
                        type_.hir(db)
                    })
                    .unwrap_or_else(|| type_param.hir(db).ty(db))
            })
            .collect()
    }
}

/// Evaluates a method's substituted where-clause in its impl's environment.
///
/// It's kind of workaround since there is no clear way how to evaluate hir
/// function bounds that discard the function based on the impl generic
/// constraints
pub(super) fn evaluate_where_clause<'db>(db: &'db dyn Db, callable: Callable<'db>) -> Option<bool> {
    let Some(impl_) = callable.function(db).impl_(db) else {
        unreachable!("impl-scoped predicate evaluation requires an associated function")
    };

    let sema = Semantics::new_dyn(db);
    let source = sema.source(callable.declaration(db).hir(db))?;
    let Some(where_clause) = source.value.where_clause() else {
        return Some(true);
    };

    let where_clause = substitute_where_clause(db, &sema, callable, where_clause)?;
    let context = impl_context(db, &sema, impl_)?;

    // Use the impl's environment so the method's own predicates are not assumed.
    let result =
        sema.evaluate_where_clause_at(&context, context.text_range().start(), where_clause);

    Some(result.status == PredicateEvaluationStatus::Holds)
}

fn impl_context<'db>(
    db: &'db dyn Db,
    sema: &Semantics<'db, dyn HirDatabase>,
    impl_: ImplQualified<'db>,
) -> Option<SyntaxNode> {
    let source = sema
        .source(impl_.declaration(db).hir(db))
        .and_then(|source| source.value.self_ty())
        .map(|type_| type_.syntax().clone());

    source.or_else(|| {
        let adt = impl_.target(db).hir(db).as_adt()?;
        let source = sema.source(adt)?.value.syntax().clone();

        Some(
            source
                .children()
                .find_map(ast::Name::cast)?
                .syntax()
                .clone(),
        )
    })
}

/// Substitutes impl type arguments and `Self` into a method's where-clause.
fn substitute_where_clause<'db>(
    db: &'db dyn Db,
    sema: &Semantics<'db, dyn HirDatabase>,
    callable: Callable<'db>,
    where_clause: ast::WhereClause,
) -> Option<ast::WhereClause> {
    let Some(impl_) = callable.function(db).impl_(db) else {
        unreachable!("impl-scoped predicate substitution requires an associated function")
    };

    let module = impl_.declaration(db).hir(db).module(db);
    let context = impl_context(db, sema, impl_)?;
    let scope = sema.scope(&context)?;
    let (editor, substituted) = SyntaxEditor::with_ast_node(&where_clause);

    // Resolve paths in the original syntax while editing its detached copy.
    let paths = where_clause
        .syntax()
        .descendants()
        .filter_map(ast::Path::cast);

    let substituted_paths = substituted
        .syntax()
        .descendants()
        .filter_map(ast::Path::cast);

    let mut replaced: Option<TextRange> = None;
    let mut trait_paths = Vec::new();

    for (path, substituted_path) in paths.zip(substituted_paths) {
        if replaced.is_some_and(|range| range.contains_range(path.syntax().text_range())) {
            continue;
        }

        let type_ = match sema.resolve_path(&path) {
            Some(PathResolution::Def(ModuleDef::Trait(trait_)))
                if matches!(impl_, ImplQualified::Trait(_)) =>
            {
                let qualified = module.find_use_path(
                    db,
                    ModuleDef::Trait(trait_),
                    PrefixKind::ByCrate,
                    FindPathConfig {
                        prefer_no_std: false,
                        prefer_prelude: false,
                        prefer_absolute: true,
                        allow_unstable: true,
                    },
                )?;
                let name_end = path.segment()?.name_ref()?.syntax().text_range().end()
                    - path.syntax().text_range().start();
                let base = path.syntax().text().to_string()[..usize::from(name_end)].to_owned();

                trait_paths.push((
                    base,
                    qualified
                        .display(db, module.krate(db).edition(db))
                        .to_string(),
                ));

                continue;
            }
            Some(PathResolution::TypeParam(param)) => callable.instantiate(db, param.ty(db)),
            Some(PathResolution::SelfType(_)) => impl_.target(db).hir(db),
            Some(PathResolution::Def(ModuleDef::TypeAlias(alias)))
                if alias.as_assoc_item(db).is_some() =>
            {
                // Rebuild projections through HIR to preserve their associated trait.
                let type_ = ast::Type::cast(path.syntax().parent()?)?;
                callable.instantiate(db, sema.resolve_type(&type_)?)
            }
            _ => continue,
        };

        if type_.contains_unknown() || !type_.generic_params(db).is_empty() {
            return None;
        }

        // Qualify inserted types so impl generic parameter names cannot shadow them.
        let type_ = make::ty(&type_.display_source_code(db, module.into(), false).ok()?);
        let mut source = type_.syntax().text().to_string();
        let offset = type_.syntax().text_range().start();
        let mut paths = Vec::new();

        for path in type_.syntax().descendants().filter_map(ast::Path::cast) {
            if path.qualifier().is_some() {
                continue;
            }

            let Some(name) = path.segment().and_then(|segment| segment.name_ref()) else {
                continue;
            };
            let Some(PathResolution::Def(def)) = scope.speculative_resolve(&path) else {
                continue;
            };
            if matches!(def, ModuleDef::BuiltinType(_)) {
                continue;
            }

            let qualified = module.find_use_path(
                db,
                def,
                PrefixKind::ByCrate,
                FindPathConfig {
                    prefer_no_std: false,
                    prefer_prelude: false,
                    prefer_absolute: true,
                    allow_unstable: true,
                },
            )?;
            paths.push((
                name.syntax().text_range() - offset,
                qualified
                    .display(db, module.krate(db).edition(db))
                    .to_string(),
            ));
        }

        // Apply text edits backwards so the remaining ranges stay valid.
        paths.sort_unstable_by_key(|(range, _)| range.start());
        for (range, path) in paths.into_iter().rev() {
            source.replace_range(usize::from(range.start())..usize::from(range.end()), &path);
        }

        let type_ = make::ty(&source);
        let parent = substituted_path.syntax().parent()?;

        if ast::PathType::can_cast(parent.kind()) {
            editor.replace(&parent, type_.syntax());
        } else if ast::Path::can_cast(parent.kind()) {
            let qualifier = make::path_unqualified(make::path_segment_ty(type_, None));
            editor.replace(substituted_path.syntax(), qualifier.syntax());
        } else {
            return None;
        }

        replaced = Some(path.syntax().text_range());
    }

    let clause = ast::WhereClause::cast(editor.finish().new_root().clone())?;
    if trait_paths.is_empty() {
        return Some(clause);
    }

    let offset = clause.syntax().text_range().start();
    let mut source = clause.syntax().text().to_string();
    let mut edits = Vec::new();

    for path in clause.syntax().descendants().filter_map(ast::Path::cast) {
        let parent = path.syntax().parent()?;
        if !ast::PathType::can_cast(parent.kind())
            || !parent
                .parent()
                .is_some_and(|parent| ast::TypeBound::can_cast(parent.kind()))
        {
            continue;
        }
        let range = TextRange::new(
            path.syntax().text_range().start(),
            path.segment()?.name_ref()?.syntax().text_range().end(),
        );
        let base = &source[usize::from(range.start() - offset)..usize::from(range.end() - offset)];
        if let Some((_, qualified)) = trait_paths.iter().find(|(original, _)| original == base) {
            edits.push((range - offset, qualified));
        }
    }

    edits.sort_unstable_by_key(|(range, _)| range.start());
    for (range, qualified) in edits.into_iter().rev() {
        source.replace_range(
            usize::from(range.start())..usize::from(range.end()),
            qualified,
        );
    }

    ast::SourceFile::parse(
        &format!("fn __rustier_where() {source} {{}}"),
        ra_ap_edition::Edition::CURRENT,
    )
    .syntax_node()
    .descendants()
    .find_map(ast::WhereClause::cast)
}
