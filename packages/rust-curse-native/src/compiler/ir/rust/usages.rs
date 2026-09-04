use std::collections::{HashMap, HashSet};

use crate::compiler::{
    db::Db,
    ir::rust::{Enum, Function, FunctionParent, Impl, Member, Module, ModuleParent, Struct, ValueType},
};

/// A usage graph that derives all used members of a rust api from a set of root
/// modules.
#[salsa::interned(debug)]
pub struct UsageGraph<'db> {
    /// The modules that seed usage graph
    #[returns(ref)]
    pub roots: Vec<Module<'db>>,
}

#[salsa::tracked]
impl<'db> UsageGraph<'db> {
    /// The reflexive transitive closure of members used by the root module
    /// members.
    #[salsa::tracked(returns(ref))]
    pub fn members(self, db: &'db dyn Db) -> HashSet<Member<'db>> {
        let mut visitor = Visitor {
            db,
            visited: HashSet::default(),
            used_members: HashSet::default(),
        };

        for root in self.roots(db) {
            visitor.visit_module(*root);
        }

        visitor.used_members
    }

    /// The members of a given module used transitively by the root module
    /// members.
    pub fn module_members(
        self,
        db: &'db dyn Db,
        module: Module<'db>,
    ) -> Option<&'db Vec<Member<'db>>> {
        self.all_module_members(db).get(&module)
    }

    #[salsa::tracked(returns(ref))]
    fn all_module_members(self, db: &'db dyn Db) -> HashMap<Module<'db>, Vec<Member<'db>>> {
        let used_members = self.members(db);
        let mut result = HashMap::default();

        for module in used_members.iter().filter_map(|member| match member {
            Member::Module(module) => Some(*module),
            _ => None,
        }) {
            result.insert(
                module,
                module
                    .members(db)
                    .iter()
                    .filter(|member| used_members.contains(member))
                    .copied()
                    .collect(),
            );
        }

        result
    }
}

struct Visitor<'db> {
    db: &'db dyn Db,
    visited: HashSet<Node<'db>>,
    used_members: HashSet<Member<'db>>,
}

impl<'db> Visitor<'db> {
    pub fn visit_module(&mut self, module: Module<'db>) {
        self.used_members.insert(Member::Module(module));

        for member in module.members(self.db) {
            match member {
                Member::Function(function) => self.visit_function(*function),
                Member::Struct(struct_) => self.visit_struct(*struct_),
                Member::Enum(enum_) => self.visit_enum(*enum_),
                Member::Module(module) => self.visit_module(*module),
            }
        }

        self.add_parent_chain(module);
    }

    pub fn visit_function(&mut self, function: Function<'db>) {
        if !self.visited.insert(Node::Function(function)) {
            return;
        }
        self.used_members.insert(Member::Function(function));

        if let FunctionParent::Module(module) = function.parent(self.db) {
            self.add_parent_chain(module);
        }

        for param in function.params(self.db) {
            self.visit_value_type(param.type_(self.db).deborrow());
        }
        if let Some(ret_type) = function.ret_type(self.db) {
            self.visit_value_type(ret_type);
        }
    }

    pub fn visit_struct(&mut self, struct_: Struct<'db>) {
        if !self.visited.insert(Node::Struct(struct_)) {
            return;
        }
        self.used_members.insert(Member::Struct(struct_));
        self.add_parent_chain(struct_.module(self.db));

        for field in struct_.fields(self.db) {
            self.visit_value_type(field.value_type(self.db));
        }
        for impl_ in struct_.impls(self.db) {
            self.visit_impl(*impl_);
        }
    }

    pub fn visit_enum(&mut self, enum_: Enum<'db>) {
        if !self.visited.insert(Node::Enum(enum_)) {
            return;
        }
        self.used_members.insert(Member::Enum(enum_));
        self.add_parent_chain(enum_.module(self.db));

        for field in enum_.fields(self.db) {
            self.visit_value_type(field.value_type(self.db));
        }
        for impl_ in enum_.impls(self.db) {
            self.visit_impl(*impl_);
        }
    }

    pub fn visit_impl(&mut self, impl_: Impl<'db>) {
        for function in impl_.functions(self.db) {
            self.visit_function(*function);
        }
    }

    fn visit_value_type(&mut self, value_type: ValueType<'db>) {
        match value_type {
            ValueType::Struct(struct_) => self.visit_struct(struct_),
            ValueType::Enum(enum_) => self.visit_enum(enum_),
            _ => {}
        }
    }

    fn add_parent_chain(&mut self, module: Module<'db>) {
        let mut current = module;
        self.used_members.insert(Member::Module(module));

        while let ModuleParent::Module(parent) = current.parent(self.db) {
            current = parent;
            self.used_members.insert(Member::Module(current));
        }
    }
}

#[derive(PartialEq, Eq, Hash)]
enum Node<'db> {
    Struct(Struct<'db>),
    Enum(Enum<'db>),
    Function(Function<'db>),
}
