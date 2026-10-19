use std::collections::{HashMap, HashSet};

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use crate::compiler::{
    db::Db,
    diag::{DiagAccumulateExt as _, DiagSource},
    ir::rust::{
        Callable, Enum, EnumType, FunctionFree, FunctionQualified, GenArg, Member, Module,
        ModuleParent, Rust, Struct, StructType, Type, TypeKind,
    },
};

/// A usage graph rooted in nongeneric public declarations.
#[salsa::interned(debug, constructor = new_with_max_depth)]
pub struct UsageGraph<'db> {
    /// The modules that seed usage discovery.
    #[returns(ref)]
    pub roots: Vec<Module<'db>>,
    /// Maximum number of type-dependency levels along a discovery path.
    pub max_depth: usize,
}

impl<'db> UsageGraph<'db> {
    pub fn new(db: &'db dyn Db, roots: impl IntoIterator<Item = Module<'db>>) -> Self {
        Self::new_with_max_depth(db, roots.into_iter().collect::<Vec<_>>(), 128)
    }
}

#[salsa::tracked]
impl<'db> UsageGraph<'db> {
    /// Seed nongeneric public declarations from every crate's root module.
    pub fn complete(db: &'db dyn Db) -> Self {
        let roots: Vec<_> = Rust::get(db)
            .crates(db)
            .iter()
            .map(|crate_| crate_.root(db))
            .collect();

        Self::new(db, roots)
    }

    /// Reachable declarations and their containing modules.
    pub fn members(self, db: &'db dyn Db) -> impl Iterator<Item = Member<'db>> + 'db {
        self.reachable(db).iter().filter_map(|node| match node {
            Node::Struct(value) => Some(Member::Struct(*value)),
            Node::Enum(value) => Some(Member::Enum(*value)),
            Node::Function(value) => Some(Member::Function(*value)),
            Node::Module(value) => Some(Member::Module(*value)),
            Node::Type(_) | Node::Callable(_) => None,
        })
    }

    /// Distinct reachable types.
    pub fn types(self, db: &'db dyn Db) -> impl Iterator<Item = Type<'db>> + 'db {
        self.reachable(db).iter().filter_map(|node| match node {
            Node::Type(type_) => Some(*type_),
            _ => None,
        })
    }

    pub fn module_members(
        self,
        db: &'db dyn Db,
        module: Module<'db>,
    ) -> Option<&'db Vec<Member<'db>>> {
        self.all_module_members(db).get(&module)
    }

    #[salsa::tracked(returns(ref))]
    fn reachable(self, db: &'db dyn Db) -> HashSet<Node<'db>> {
        let mut visitor = Visitor {
            db,
            reachable: HashSet::default(),
            visited_modules: HashSet::default(),
            visited_types: HashMap::default(),
            visited_callables: HashMap::default(),
            depth_failures: HashMap::default(),
            max_depth: self.max_depth(db),
        };

        for root in self.roots(db) {
            visitor.visit_module(*root);
        }

        for type_ in visitor.depth_failures.into_values() {
            let span = type_.origin(db).span(db);
            DiscoveryDepthExceeded {
                type_: type_.display(db).to_owned(),
                limit: self.max_depth(db),
                source_code: span.map(|span| span.src(db).into()).unwrap_or_default(),
                span: span.map(|span| (span.offset, span.length).into()),
            }
            .accumulate(db);
        }
        visitor.reachable
    }

    #[salsa::tracked(returns(ref))]
    fn all_module_members(self, db: &'db dyn Db) -> HashMap<Module<'db>, Vec<Member<'db>>> {
        let members: HashSet<_> = self.members(db).collect();
        let mut result = HashMap::default();

        for module in members.iter().filter_map(|member| match member {
            Member::Module(module) => Some(*module),
            _ => None,
        }) {
            result.insert(
                module,
                module
                    .members(db)
                    .iter()
                    .filter(|member| members.contains(member))
                    .copied()
                    .collect(),
            );
        }

        result
    }
}

struct Visitor<'db> {
    db: &'db dyn Db,
    reachable: HashSet<Node<'db>>,
    visited_modules: HashSet<Module<'db>>,
    // FIXME: HIR equality ignores source origins and distinguishes actual instantiations, but
    // rust::Type does not.
    visited_types: HashMap<ra_ap_hir::Type<'db>, (Type<'db>, usize)>,
    visited_callables: HashMap<Callable<'db>, usize>,
    depth_failures: HashMap<ra_ap_hir::Type<'db>, Type<'db>>,
    max_depth: usize,
}

impl<'db> Visitor<'db> {
    fn visit_module(&mut self, module: Module<'db>) {
        if !self.visited_modules.insert(module) {
            return;
        }
        self.add_parent_chain(module);

        for member in module.members(self.db) {
            match member {
                Member::Function(function) => {
                    if let Some(callable) = function.as_qualified().as_callible(self.db) {
                        self.visit_callable(callable, 0);
                    }
                }
                Member::Struct(struct_) => {
                    if let Some(type_) = struct_.as_mono_type(self.db) {
                        self.visit_type(type_, 0);
                    }
                }
                Member::Enum(enum_) => {
                    if let Some(type_) = enum_.as_mono_type(self.db) {
                        self.visit_type(type_, 0);
                    }
                }
                Member::Module(module) => self.visit_module(*module),
            }
        }
    }

    fn visit_callable(&mut self, callable: Callable<'db>, depth: usize) {
        if self
            .visited_callables
            .get(&callable)
            .is_some_and(|previous| *previous <= depth)
        {
            return;
        }
        self.visited_callables.insert(callable, depth);
        self.reachable.insert(Node::Callable(callable));

        if let FunctionQualified::Free(function) = callable.function(self.db) {
            self.reachable.insert(Node::Function(function));
        }

        if let Some(receiver) = callable.receiver_type(self.db) {
            self.visit_type(receiver, depth);
        }
        for param in callable.params(self.db) {
            self.visit_type(*param.type_(self.db), depth);
        }
        let ret_type = callable.ret_type(self.db);
        let (ret_type, ret_err_type) = match ret_type.as_result_variants(self.db) {
            Some((ok, err)) => (ok, Some(err)),
            None => (ret_type, None),
        };

        self.visit_type(ret_type, depth);

        if let Some(err) = ret_err_type {
            self.visit_type(err, depth);
        }
    }

    fn visit_struct(&mut self, struct_: StructType<'db>, depth: usize) {
        self.reachable.insert(Node::Struct(struct_.def(self.db)));
        self.add_parent_chain(struct_.def(self.db).module(self.db));
        self.visit_args(struct_.gen_args(self.db), depth + 1);

        for field in struct_.fields(self.db) {
            self.visit_type(field._type(self.db), depth + 1);
        }
        self.visit_impls(struct_.into_type(self.db), depth + 1);
    }

    fn visit_enum(&mut self, enum_: EnumType<'db>, depth: usize) {
        self.reachable.insert(Node::Enum(enum_.def(self.db)));
        self.add_parent_chain(enum_.def(self.db).module(self.db));
        self.visit_args(enum_.gen_args(self.db), depth + 1);

        for variant in enum_.variants(self.db) {
            for field in variant.fields(self.db) {
                self.visit_type(field._type(self.db), depth + 1);
            }
        }
        self.visit_impls(enum_.into_type(self.db), depth + 1);
    }

    fn visit_impls(&mut self, type_: Type<'db>, depth: usize) {
        for impl_ in type_.impls(self.db) {
            for function in impl_.functions(self.db) {
                if let Some(callable) = function.as_callible(self.db) {
                    self.visit_callable(callable, depth);
                }
            }
        }
    }

    fn visit_type(&mut self, mut type_: Type<'db>, depth: usize) {
        if type_.is_js_env(self.db) {
            return;
        }

        let hir = type_.hir(self.db);
        if let Some(&(representative, previous_depth)) = self.visited_types.get(&hir) {
            if previous_depth <= depth {
                return;
            }
            type_ = representative;
        }
        if depth >= self.max_depth {
            self.depth_failures.entry(hir).or_insert(type_);
            return;
        }
        self.depth_failures.remove(&hir);
        self.visited_types.insert(hir, (type_, depth));
        self.reachable.insert(Node::Type(type_));
        match type_.kind(self.db) {
            TypeKind::Struct(struct_) => self.visit_struct(struct_, depth),
            TypeKind::Enum(enum_) => self.visit_enum(enum_, depth),
            TypeKind::Borrowed(reference) => self.visit_type(reference.inner(self.db), depth + 1),
            _ => {}
        }
    }

    fn visit_args(&mut self, args: &[GenArg<'db>], depth: usize) {
        for arg in args {
            let GenArg::Type(type_) = arg;
            self.visit_type(*type_, depth);
        }
    }

    fn add_parent_chain(&mut self, module: Module<'db>) {
        let mut current = module;
        self.reachable.insert(Node::Module(module));

        while let ModuleParent::Module(parent) = current.parent(self.db) {
            current = parent;
            self.reachable.insert(Node::Module(current));
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
enum Node<'db> {
    Struct(Struct<'db>),
    Enum(Enum<'db>),
    Function(FunctionFree<'db>),
    Module(Module<'db>),
    Type(Type<'db>),
    Callable(Callable<'db>),
}

#[derive(Error, Diagnostic, Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
#[error("maximum discovery depth of {limit} exceeded while expanding \"{type_}\"")]
#[diagnostic(code(rustier::usage_expansion_limit), severity(Warning))]
struct DiscoveryDepthExceeded {
    type_: String,
    limit: usize,
    #[source_code]
    source_code: DiagSource,
    #[label("this dependency exceeds the discovery depth limit")]
    span: Option<SourceSpan>,
}
