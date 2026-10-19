//! Intermediate representation used to generate the JavaScript API.

pub mod coerce;
pub mod decls;
pub mod ext;
pub mod imports_js;
pub mod imports_ts;
pub mod type_js;
pub mod type_ts;

use std::collections::{BTreeMap, VecDeque};

use itertools::{Either, Itertools};

use crate::compiler::{
    db::Db,
    ir::{
        runtime::RuntimeModule,
        rust::{
            Callable, Crate, Enum, EnumType, FieldName, FieldQualified, FunctionQualified, GenArg,
            Member, Module, Rust, Span, Struct, StructType, Type, TypeKind, VariantQualified,
            usages::UsageGraph,
        },
    },
    path::FilePath,
};

/// The facade for a complete reachable Rust API.
#[salsa::interned(debug)]
pub struct Facade<'db> {
    pub rust: Rust<'db>,
    pub usages: UsageGraph<'db>,
}

#[salsa::tracked]
impl<'db> Facade<'db> {
    pub fn modules(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeModuleEntry<'db>> + 'db {
        std::iter::once(FacadeModuleEntry::Runtime(RuntimeModule)).chain(
            self.crates(db)
                .flat_map(move |crate_| crate_.all_modules(db))
                .map(FacadeModuleEntry::Source),
        )
    }

    pub fn crates(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeCrate<'db>> {
        self.rust(db)
            .crates(db)
            .iter()
            .map(move |crate_| FacadeCrate::new(db, *crate_, self.usages(db)))
    }

    #[salsa::tracked(returns(ref))]
    pub fn local_modules(self, db: &'db dyn Db) -> Vec<FacadeModule<'db>> {
        self.modules_by_locality(db, true)
            .into_iter()
            .filter_map(|module| match module {
                FacadeModuleEntry::Source(module) => Some(module),
                FacadeModuleEntry::Runtime(_) => None, // always external
            })
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub fn external_modules(self, db: &'db dyn Db) -> Vec<FacadeModuleEntry<'db>> {
        self.modules_by_locality(db, false)
    }

    fn modules_by_locality(self, db: &'db dyn Db, is_local: bool) -> Vec<FacadeModuleEntry<'db>> {
        let mut modules = self
            .modules(db)
            .filter(|module| module.is_local(db) == is_local)
            .filter(|module| match module {
                FacadeModuleEntry::Source(module) => module.members(db).iter().any(|member| {
                    matches!(
                        member,
                        FacadeMember::Class(_)
                            | FacadeMember::ClassTemplate(_)
                            | FacadeMember::Function(_)
                    )
                }),
                FacadeModuleEntry::Runtime(_) => true,
            })
            .collect::<Vec<_>>();

        modules.sort_by_key(|module| module.virtual_path(db));

        modules
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum FacadeModuleEntry<'db> {
    Source(FacadeModule<'db>),
    Runtime(RuntimeModule),
}

impl<'db> FacadeModuleEntry<'db> {
    pub fn is_local(self, db: &'db dyn Db) -> bool {
        match self {
            Self::Source(module) => module.as_parent(db).is_local(db),
            Self::Runtime(_) => false,
        }
    }

    pub fn virtual_path(self, db: &'db dyn Db) -> &'db str {
        match self {
            Self::Source(module) => module.virtual_path(db),
            Self::Runtime(module) => module.virtual_path(),
        }
    }

    pub fn file_path(self, db: &'db dyn Db) -> Option<&'db FilePath> {
        match self {
            Self::Source(module) => Some(module.file_path(db)),
            Self::Runtime(_) => None,
        }
    }
}

#[salsa::interned(debug)]
pub struct FacadeCrate<'db> {
    pub src: Crate<'db>,
    pub usages: UsageGraph<'db>,
}

impl<'db> FacadeCrate<'db> {
    pub fn is_local(self, db: &'db dyn Db) -> bool {
        self.src(db).is_local(db)
    }

    pub fn root(self, db: &'db dyn Db) -> FacadeModule<'db> {
        self.src(db)
            .root(db)
            .facade_reachable_from(db, self.usages(db))
    }

    pub fn all_modules(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeModule<'db>> + 'db {
        let mut pending = VecDeque::from([self.root(db)]);

        std::iter::from_fn(move || {
            let module = pending.pop_front()?;
            pending.extend(module.modules(db));
            Some(module)
        })
    }
}

/// A facade module used as a parent of another facade entity.
///
/// Matches a single `.js` file generated for a rust module. Take into accout
/// that there is no crate containing this module since crates aren't a thing in
/// js.
#[salsa::interned(debug)]
pub struct FacadeParentModule<'db> {
    pub src: Module<'db>,
}

#[salsa::tracked]
impl<'db> FacadeParentModule<'db> {
    pub fn is_local(self, db: &'db dyn Db) -> bool {
        self.src(db).path(db).crate_(db).is_local(db)
    }

    /// Stable module specifier independent of source file layout.
    #[salsa::tracked(returns(ref))]
    pub fn virtual_path(self, db: &'db dyn Db) -> String {
        let path = self.src(db).path(db);
        let names = path
            .segments(db)
            .iter()
            .map(|segment| segment.name(db).as_str())
            .collect::<Vec<_>>();

        format!("$rust/{}", names.join("/"))
    }

    /// File path of the source rust module.
    pub fn file_path(self, db: &'db dyn Db) -> &'db FilePath {
        self.src(db).file_path(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.src(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.src(db).span(db)
    }
}

/// A reachable facade module.
#[salsa::interned(debug)]
pub struct FacadeModule<'db> {
    pub src: Module<'db>,
    pub usages: UsageGraph<'db>,
}

#[salsa::tracked]
impl<'db> FacadeModule<'db> {
    pub fn virtual_path(self, db: &'db dyn Db) -> &'db String {
        self.as_parent(db).virtual_path(db)
    }

    pub fn as_parent(self, db: &'db dyn Db) -> FacadeParentModule<'db> {
        self.src(db).facade_parent(db)
    }

    /// All members defined in this module.
    #[salsa::tracked(returns(ref))]
    pub fn members(self, db: &'db dyn Db) -> Vec<FacadeMember<'db>> {
        let usages = self.usages(db);

        let Some(members) = usages.module_members(db, self.src(db)) else {
            return vec![];
        };

        let mut lowered = vec![];

        for member in members {
            match *member {
                Member::Struct(src) if !src.generic_params(db).is_empty() => {
                    let template = FacadeClassPlainTemplate::new(db, src, usages);

                    lowered.push(FacadeMember::ClassTemplate(FacadeClassTemplate::Plain(
                        template,
                    )));
                }
                Member::Enum(src) if !src.generic_params(db).is_empty() => {
                    let template = FacadeClassDiscriminatedTemplate::new(db, src, usages);

                    lowered.push(FacadeMember::ClassTemplate(
                        FacadeClassTemplate::Discriminated(template),
                    ));
                }
                Member::Struct(_) | Member::Enum(_) => lowered.extend(
                    usages
                        .types(db)
                        .filter_map(|type_| match (*member, type_.kind(db)) {
                            (Member::Struct(src), TypeKind::Struct(type_))
                                if type_.def(db) == src =>
                            {
                                Some(FacadeClass::Plain(type_.facade(db)))
                            }
                            (Member::Enum(src), TypeKind::Enum(type_)) if type_.def(db) == src => {
                                Some(FacadeClass::Discriminated(type_.facade(db)))
                            }
                            _ => None,
                        })
                        .map(FacadeMember::Class),
                ),
                Member::Function(function) => {
                    if let Some(callable) = function.as_qualified().as_callible(db) {
                        lowered.push(FacadeMember::Function(FacadeFreeFunction::new(
                            db, callable,
                        )));
                    }
                }
                Member::Module(module) => lowered.push(FacadeMember::Module(
                    module.facade_reachable_from(db, usages),
                )),
            }
        }

        lowered
    }

    pub fn classes(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeClass<'db>> + 'db {
        self.members(db)
            .iter()
            .copied()
            .flat_map(move |member| match member {
                FacadeMember::Class(class) => Either::Left(Some(class).into_iter()),
                FacadeMember::ClassTemplate(template) => Either::Right(template.classes(db)),
                _ => Either::Left(None.into_iter()),
            })
    }

    pub fn functions(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeFreeFunction<'db>> + 'db {
        self.members(db)
            .iter()
            .copied()
            .filter_map(|member| match member {
                FacadeMember::Function(function) => Some(function),
                _ => None,
            })
    }

    pub fn modules(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeModule<'db>> + 'db {
        self.members(db)
            .iter()
            .copied()
            .filter_map(|member| match member {
                FacadeMember::Module(module) => Some(module),
                _ => None,
            })
    }

    /// File path of the source rust module.
    pub fn file_path(self, db: &'db dyn Db) -> &'db FilePath {
        self.as_parent(db).file_path(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.src(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.src(db).span(db)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeMember<'db> {
    Class(FacadeClass<'db>),
    ClassTemplate(FacadeClassTemplate<'db>),
    Function(FacadeFreeFunction<'db>),
    Module(FacadeModule<'db>),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeClassTemplate<'db> {
    Plain(FacadeClassPlainTemplate<'db>),
    Discriminated(FacadeClassDiscriminatedTemplate<'db>),
}

#[salsa::tracked]
impl<'db> FacadeClassTemplate<'db> {
    pub fn module(self, db: &'db dyn Db) -> FacadeParentModule<'db> {
        match self {
            Self::Plain(template) => template.src(db).module(db).facade_parent(db),
            Self::Discriminated(template) => template.src(db).module(db).facade_parent(db),
        }
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self {
            Self::Plain(template) => template.src(db).segment(db).name(db),
            Self::Discriminated(template) => template.src(db).segment(db).name(db),
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn param_names(self, db: &'db dyn Db) -> Vec<String> {
        let params = match self {
            Self::Plain(template) => template.src(db).generic_params(db),
            Self::Discriminated(template) => template.src(db).generic_params(db),
        };

        params
            .iter()
            .map(|param| {
                param
                    .name(db)
                    .expect("ADT type parameters are named")
                    .to_owned()
            })
            .collect()
    }

    pub fn classes(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeClass<'db>> + 'db {
        match self {
            Self::Plain(template) => Either::Left(template.classes(db).map(FacadeClass::Plain)),
            Self::Discriminated(template) => {
                Either::Right(template.classes(db).map(FacadeClass::Discriminated))
            }
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn mappings(self, db: &'db dyn Db) -> Vec<(Vec<FacadeGeneric<'db>>, FacadeClass<'db>)> {
        let mut entries: Vec<_> = self
            .classes(db)
            .map(|class| (class.generics(db).collect(), class))
            .collect();

        entries.sort_by(|(_, left), (_, right)| left.name(db).cmp(right.name(db)));

        entries
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        match self {
            Self::Plain(template) => template.src(db).docs(db),
            Self::Discriminated(template) => template.src(db).docs(db),
        }
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        match self {
            Self::Plain(template) => template.src(db).span(db),
            Self::Discriminated(template) => template.src(db).span(db),
        }
    }
}

#[salsa::interned(debug)]
pub struct FacadeClassPlainTemplate<'db> {
    pub src: Struct<'db>,
    pub usages: UsageGraph<'db>,
}

impl<'db> FacadeClassPlainTemplate<'db> {
    pub fn classes(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeClassPlain<'db>> + 'db {
        let src = self.src(db);

        self.usages(db)
            .types(db)
            .filter_map(move |type_| match type_.kind(db) {
                TypeKind::Struct(type_) if type_.def(db) == src => Some(type_.facade(db)),
                _ => None,
            })
    }
}

#[salsa::interned(debug)]
pub struct FacadeClassDiscriminatedTemplate<'db> {
    pub src: Enum<'db>,
    pub usages: UsageGraph<'db>,
}

impl<'db> FacadeClassDiscriminatedTemplate<'db> {
    pub fn classes(
        self,
        db: &'db dyn Db,
    ) -> impl Iterator<Item = FacadeClassDiscriminated<'db>> + 'db {
        let src = self.src(db);

        self.usages(db)
            .types(db)
            .filter_map(move |type_| match type_.kind(db) {
                TypeKind::Enum(type_) if type_.def(db) == src => Some(type_.facade(db)),
                _ => None,
            })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, salsa::Update)]
pub enum FacadeGeneric<'db> {
    Symbol(String),
    Class(FacadeClass<'db>),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeFunction<'db> {
    Free(FacadeFreeFunction<'db>),
    Method(FacadeMethod<'db>),
}

#[salsa::tracked]
impl<'db> FacadeFunction<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        match self {
            Self::Free(function) => function.name(db),
            Self::Method(method) => method.name(db),
        }
    }

    pub fn params(self, db: &'db dyn Db) -> &'db Vec<FacadeParam<'db>> {
        match self {
            Self::Free(function) => function.params(db),
            Self::Method(method) => method.params(db),
        }
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Type<'db> {
        match self {
            Self::Free(function) => function.ret_type(db),
            Self::Method(method) => method.ret_type(db),
        }
    }

    pub fn ret_err_type(self, db: &'db dyn Db) -> Option<Type<'db>> {
        match self {
            Self::Free(function) => function.ret_err_type(db),
            Self::Method(method) => method.ret_err_type(db),
        }
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        match self {
            Self::Free(function) => function.docs(db),
            Self::Method(method) => method.docs(db),
        }
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        match self {
            Self::Free(function) => function.span(db),
            Self::Method(method) => method.span(db),
        }
    }
}

#[salsa::interned(debug)]
pub struct FacadeFreeFunction<'db> {
    pub src: Callable<'db>,
}

#[salsa::tracked]
impl<'db> FacadeFreeFunction<'db> {
    pub fn as_fn(self) -> FacadeFunction<'db> {
        FacadeFunction::Free(self)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.src(db).declaration(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.src(db).declaration(db).span(db)
    }

    #[salsa::tracked(returns(ref))]
    pub fn name(self, db: &'db dyn Db) -> String {
        let name = self.src(db).declaration(db).name(db);

        name.strip_prefix("r#").unwrap_or(name).to_owned()
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<FacadeParam<'db>> {
        self.src(db)
            .params(db)
            .iter()
            .enumerate()
            .filter(|(_, param)| !param.type_(db).is_js_env(db))
            .map(|(idx, param)| FacadeParam::new(db, self.as_fn(), idx, *param.type_(db)))
            .collect()
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Type<'db> {
        let type_ = self.src(db).ret_type(db);

        type_.as_result_variants(db).map_or(type_, |(ok, _)| ok)
    }

    pub fn ret_err_type(self, db: &'db dyn Db) -> Option<Type<'db>> {
        self.src(db)
            .ret_type(db)
            .as_result_variants(db)
            .map(|(_, err)| err)
    }
}

#[salsa::interned(debug)]
pub struct FacadeMethod<'db> {
    pub src: Callable<'db>,
}

#[salsa::tracked]
impl<'db> FacadeMethod<'db> {
    pub fn as_fn(self) -> FacadeFunction<'db> {
        FacadeFunction::Method(self)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.src(db).declaration(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.src(db).declaration(db).span(db)
    }

    pub fn is_static(self, db: &'db dyn Db) -> bool {
        self.src(db).declaration(db).receiver(db).is_none()
    }

    pub fn class(self, db: &'db dyn Db) -> FacadeClass<'db> {
        let FunctionQualified::Inherent(function) = self.src(db).function(db) else {
            unreachable!("facade methods must be inherent")
        };
        let Some(class) = function.impl_(db).target(db).facade_class(db) else {
            unreachable!("facade methods must target a class")
        };

        class
    }

    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.src(db).declaration(db).name(db)
    }

    #[salsa::tracked(returns(ref))]
    pub fn params(self, db: &'db dyn Db) -> Vec<FacadeParam<'db>> {
        let offset = usize::from(!self.is_static(db));

        self.src(db)
            .params(db)
            .iter()
            .enumerate()
            .filter(|(_, param)| !param.type_(db).is_js_env(db))
            .map(|(idx, param)| FacadeParam::new(db, self.as_fn(), idx + offset, *param.type_(db)))
            .collect()
    }

    pub fn ret_type(self, db: &'db dyn Db) -> Type<'db> {
        let type_ = self.src(db).ret_type(db);

        type_.as_result_variants(db).map_or(type_, |(ok, _)| ok)
    }

    pub fn ret_err_type(self, db: &'db dyn Db) -> Option<Type<'db>> {
        self.src(db)
            .ret_type(db)
            .as_result_variants(db)
            .map(|(_, err)| err)
    }
}

#[salsa::tracked(debug)]
pub struct FacadeParam<'db> {
    pub parent: FacadeFunction<'db>,
    pub idx: usize,
    #[tracked]
    pub src_type: Type<'db>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, salsa::Update, salsa::Supertype)]
pub enum FacadeClass<'db> {
    Plain(FacadeClassPlain<'db>),
    Discriminated(FacadeClassDiscriminated<'db>),
}

#[salsa::tracked]
impl<'db> FacadeClass<'db> {
    pub fn src_type(self, db: &'db dyn Db) -> Type<'db> {
        match self {
            Self::Plain(class) => class.src(db).into_type(db),
            Self::Discriminated(class) => class.src(db).into_type(db),
        }
    }

    pub fn generics(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeGeneric<'db>> + 'db {
        let args = match self {
            Self::Plain(class) => class.src(db).gen_args(db),
            Self::Discriminated(class) => class.src(db).gen_args(db),
        };

        args.iter()
            .map(move |GenArg::Type(type_)| match type_.facade_class(db) {
                Some(class) => FacadeGeneric::Class(class),
                None => FacadeGeneric::Symbol(type_.display(db).to_owned()),
            })
    }

    pub fn source_fields(self, db: &'db dyn Db) -> &'db Vec<FacadeFieldPlain<'db>> {
        match self {
            Self::Plain(class) => class.source_fields(db),
            Self::Discriminated(class) => class.source_fields(db),
        }
    }

    pub fn module(self, db: &'db dyn Db) -> FacadeParentModule<'db> {
        match self {
            FacadeClass::Plain(plain) => plain.module(db),
            FacadeClass::Discriminated(discriminated) => discriminated.module(db),
        }
    }

    pub fn name(self, db: &'db dyn Db) -> &'db str {
        match self {
            FacadeClass::Plain(plain) => plain.name(db),
            FacadeClass::Discriminated(discriminated) => discriminated.name(db),
        }
    }

    pub fn methods(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeMethod<'db>> + 'db {
        match self {
            FacadeClass::Plain(plain) => Either::Left(plain.methods(db)),
            FacadeClass::Discriminated(discriminated) => Either::Right(discriminated.methods(db)),
        }
    }

    pub fn constructor(self, db: &'db dyn Db) -> Option<FacadeMethod<'db>> {
        match self {
            FacadeClass::Plain(plain) => plain.constructor(db),
            FacadeClass::Discriminated(_) => None,
        }
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        match self {
            Self::Plain(class) => class.docs(db),
            Self::Discriminated(class) => class.docs(db),
        }
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        match self {
            Self::Plain(class) => class.span(db),
            Self::Discriminated(class) => class.span(db),
        }
    }
}

#[salsa::interned(debug)]
pub struct FacadeClassPlain<'db> {
    pub src: StructType<'db>,
}

#[salsa::tracked]
impl<'db> FacadeClassPlain<'db> {
    pub fn module(self, db: &'db dyn Db) -> FacadeParentModule<'db> {
        self.src(db).def(db).module(db).facade_parent(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.src(db).def(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.src(db).def(db).span(db)
    }

    #[salsa::tracked(returns(ref))]
    pub fn name(self, db: &'db dyn Db) -> String {
        let segment = self.src(db).segment(db);

        if segment.gen_args(db).is_empty() {
            segment.name(db).clone()
        } else {
            format!("_{}", segment.mangled(db))
        }
    }

    pub fn methods(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeMethod<'db>> + 'db {
        self.source_methods(db).iter().copied()
    }

    #[salsa::tracked(returns(ref))]
    pub fn source_methods(self, db: &'db dyn Db) -> Vec<FacadeMethod<'db>> {
        self.src(db)
            .into_type(db)
            .impls(db)
            .iter()
            .flat_map(|impl_| impl_.functions(db))
            .filter_map(|function| function.as_callible(db))
            .map(|callable| FacadeMethod::new(db, callable))
            .collect()
    }

    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeFieldPlain<'db>> + 'db {
        self.source_fields(db).iter().copied()
    }

    #[salsa::tracked(returns(ref))]
    pub fn source_fields(self, db: &'db dyn Db) -> Vec<FacadeFieldPlain<'db>> {
        self.src(db)
            .fields(db)
            .map(|field| field.facade(db))
            .collect()
    }

    pub fn is_constructible(self, db: &'db dyn Db) -> bool {
        self.src(db).is_constructible(db)
    }

    pub fn constructor(self, db: &'db dyn Db) -> Option<FacadeMethod<'db>> {
        self.methods(db).find(|method| {
            method.name(db) == "new"
                && method.is_static(db)
                && method
                    .src(db)
                    .ret_type(db)
                    .same_type_as(db, self.src(db).into_type(db))
        })
    }
}

#[salsa::interned(debug)]
pub struct FacadeClassDiscriminated<'db> {
    pub src: EnumType<'db>,
}

#[salsa::tracked]
impl<'db> FacadeClassDiscriminated<'db> {
    pub fn module(self, db: &'db dyn Db) -> FacadeParentModule<'db> {
        self.src(db).def(db).module(db).facade_parent(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.src(db).def(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.src(db).def(db).span(db)
    }

    #[salsa::tracked(returns(ref))]
    pub fn name(self, db: &'db dyn Db) -> String {
        let segment = self.src(db).segment(db);

        if segment.gen_args(db).is_empty() {
            segment.name(db).clone()
        } else {
            format!("_{}", segment.mangled(db))
        }
    }

    pub fn methods(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeMethod<'db>> + 'db {
        self.source_methods(db).iter().copied()
    }

    #[salsa::tracked(returns(ref))]
    pub fn source_methods(self, db: &'db dyn Db) -> Vec<FacadeMethod<'db>> {
        self.src(db)
            .into_type(db)
            .impls(db)
            .iter()
            .flat_map(|impl_| impl_.functions(db))
            .filter_map(|function| function.as_callible(db))
            .map(|callable| FacadeMethod::new(db, callable))
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub fn variants(self, db: &'db dyn Db) -> Vec<FacadeVariant<'db>> {
        self.src(db)
            .variants(db)
            .enumerate()
            .map(|(i, variant)| FacadeVariant::new(db, self, variant, i as u32))
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub fn source_fields(self, db: &'db dyn Db) -> Vec<FacadeFieldPlain<'db>> {
        self.variants(db)
            .iter()
            .flat_map(|variant| variant.fields(db))
            .collect()
    }

    pub fn is_coercable(self, db: &'db dyn Db) -> bool {
        self.variants(db)
            .iter()
            .any(|variant| variant.is_constructible(db))
    }

    #[salsa::tracked(returns(ref))]
    pub fn fields(self, db: &'db dyn Db) -> Vec<FacadeFieldDiscriminated<'db>> {
        self.variants(db)
            .iter()
            .copied()
            .flat_map(|variant| variant.fields(db).map(move |field| (field, variant)))
            .into_group_map_by(|(field, _)| field.name(db))
            .into_iter()
            .sorted_by_key(|(name, _)| name.to_string())
            .map(|(name, pairs)| {
                FacadeFieldDiscriminated::new(
                    db,
                    self,
                    name.clone(),
                    pairs
                        .into_iter()
                        .map(|(field, variant)| (variant.tag_value(db), field))
                        .collect(),
                )
            })
            .collect()
    }

    pub fn discriminator(self, db: &'db dyn Db) -> FacadeFieldDiscriminator<'db> {
        FacadeFieldDiscriminator::new(db, self)
    }
}

#[salsa::tracked(debug)]
pub struct FacadeVariant<'db> {
    pub class: FacadeClassDiscriminated<'db>,
    pub src: VariantQualified<'db>,
    pub tag_value: u32,
}

impl<'db> FacadeVariant<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db String {
        self.src(db).declaration(db).name(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.src(db).declaration(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.src(db).declaration(db).span(db)
    }

    pub fn fields(self, db: &'db dyn Db) -> impl Iterator<Item = FacadeFieldPlain<'db>> + 'db {
        self.src(db).fields(db).map(move |field| field.facade(db))
    }

    pub fn is_constructible(self, db: &'db dyn Db) -> bool {
        self.src(db).is_constructible(db)
    }
}

#[salsa::interned(debug)]
pub struct FacadeFieldPlain<'db> {
    pub src: FieldQualified<'db>,
}

impl<'db> FacadeFieldPlain<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db FieldName {
        self.src(db).declaration(db).name(db)
    }

    pub fn docs(self, db: &'db dyn Db) -> Option<&'db String> {
        self.src(db).declaration(db).docs(db)
    }

    pub fn span(self, db: &'db dyn Db) -> Option<&'db Span> {
        self.src(db).declaration(db).span(db)
    }

    pub fn is_readonly(self, db: &'db dyn Db) -> bool {
        self.src(db).is_readonly(db)
    }

    pub fn src_type(self, db: &'db dyn Db) -> Type<'db> {
        self.src(db)._type(db)
    }
}

/// A field which value depends on the discriminator tag value.
#[salsa::tracked(debug)]
pub struct FacadeFieldDiscriminated<'db> {
    pub class: FacadeClassDiscriminated<'db>,
    #[returns(ref)]
    pub name: FieldName,
    /// A map from discriminator tag value to the field.
    #[returns(ref)]
    #[tracked]
    pub sub: BTreeMap<u32, FacadeFieldPlain<'db>>,
}

/// Discriminator field of a [`FacadeClassDiscriminated`].
#[salsa::interned(debug)]
pub struct FacadeFieldDiscriminator<'db> {
    pub class: FacadeClassDiscriminated<'db>,
}

#[salsa::tracked]
impl<'db> FacadeFieldDiscriminator<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'static str {
        let enum_ = self.class(db).src(db);

        if enum_.variants(db).any(|variant| {
            variant
                .fields(db)
                .any(|field| matches!(field.declaration(db).name(db), FieldName::Name(name) if name == "type"))
        }) {
            "$type"
        } else {
            "type"
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn variant_names(self, db: &'db dyn Db) -> Vec<String> {
        self.class(db)
            .variants(db)
            .iter()
            .map(|variant| variant.name(db).clone())
            .collect()
    }
}
