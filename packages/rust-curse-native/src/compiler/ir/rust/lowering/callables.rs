use ra_ap_hir::{
    AsAssocItem as _, FindPathConfig, HirDisplay as _, ModuleDef, PathResolution,
    PredicateEvaluationStatus, PrefixKind, Semantics, db::HirDatabase,
};
use ra_ap_syntax::{
    AstNode as _, TextRange,
    ast::{self, HasGenericParams as _, make},
    syntax_editor::SyntaxEditor,
};

use crate::compiler::{
    db::Db,
    ir::rust::{
        Callable, Crate, FunctionInherent, FunctionQualified, GenArg, GenericParam, ImplInherent,
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
    pub(in crate::compiler::ir::rust) fn lower_ret_type(
        self,
        db: &'db dyn Db,
    ) -> Option<Type<'db>> {
        if !self.needs_substitution(db) {
            return self.declaration(db).ret_type(db);
        }
        let type_ = Type::new(db, TypeOrigin::CallableReturn(self));
        (!type_.hir(db).is_unit()).then_some(type_)
    }

    pub(super) fn instantiate(
        self,
        db: &'db dyn Db,
        type_: ra_ap_hir::Type<'db>,
    ) -> ra_ap_hir::Type<'db> {
        let mut args = Vec::new();
        if let FunctionQualified::Inherent(function) = self.function(db) {
            args.extend(function.impl_(db).partial_type_args(db));
        }

        args.extend(self.generic_args(db).iter().map(|arg| {
            let GenArg::Type(type_) = arg;
            type_.hir(db)
        }));

        super::instantiate_type(db, type_, args)
    }
}

impl<'db> ImplInherent<'db> {
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
fn evaluate_where_clause<'db>(db: &'db dyn Db, callable: Callable<'db>) -> Option<bool> {
    let FunctionQualified::Inherent(function) = callable.function(db) else {
        return None;
    };

    let sema = Semantics::new_dyn(db);
    let source = sema.source(function.declaration(db).hir(db))?;
    let Some(where_clause) = source.value.where_clause() else {
        return Some(true);
    };

    let where_clause = substitute_where_clause(db, &sema, callable, where_clause)?;
    let impl_source = sema.source(function.impl_(db).declaration(db).hir(db))?;
    let context = impl_source.value.self_ty()?;

    // Use the impl's environment so the method's own predicates are not assumed.
    let result = sema.evaluate_where_clause_at(
        context.syntax(),
        context.syntax().text_range().start(),
        where_clause,
    );

    Some(result.status == PredicateEvaluationStatus::Holds)
}

/// Substitutes impl type arguments and `Self` into a method's where-clause.
fn substitute_where_clause<'db>(
    db: &'db dyn Db,
    sema: &Semantics<'db, dyn HirDatabase>,
    callable: Callable<'db>,
    where_clause: ast::WhereClause,
) -> Option<ast::WhereClause> {
    let FunctionQualified::Inherent(function) = callable.function(db) else {
        return None;
    };

    let module = function.declaration(db).hir(db).module(db);
    let impl_source = sema.source(function.impl_(db).declaration(db).hir(db))?;
    let scope = sema.scope(&impl_source.value.syntax().parent()?)?;
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

    for (path, substituted_path) in paths.zip(substituted_paths) {
        if replaced.is_some_and(|range| range.contains_range(path.syntax().text_range())) {
            continue;
        }

        let type_ = match sema.resolve_path(&path) {
            Some(PathResolution::TypeParam(param)) => callable.instantiate(db, param.ty(db)),
            Some(PathResolution::SelfType(_)) => function.impl_(db).target(db).hir(db),
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

    ast::WhereClause::cast(editor.finish().new_root().clone())
}
