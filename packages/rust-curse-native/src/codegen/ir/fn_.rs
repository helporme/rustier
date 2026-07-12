use crate::codegen::{
    db::Db,
    ir::{IrId, mangle::mangle_path, type_::Type},
};

/// How a parameter is passed to a function.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, salsa::Update)]
pub enum ParamMode {
    Ref,
    Mut,
    Move,
}

/// A function parameter.
#[salsa::tracked(debug)]
pub struct FnParam<'db> {
    #[returns(ref)]
    pub name: String,
    #[tracked]
    pub type_: Type<'db>,
    #[returns(copy)]
    pub mode: ParamMode,
}

#[salsa::tracked]
impl<'db> FnParam<'db> {
    /// Returns the binding name of the parameter, e.g. `__p__foo`.
    #[salsa::tracked(returns(ref))]
    pub fn binding_name(self, db: &'db dyn Db) -> String {
        format!("__p__{}", self.name(db))
    }
}

/// A function.
#[salsa::tracked(debug)]
pub struct Fn<'db> {
    pub id: IrId<'db>,
    #[tracked]
    #[returns(ref)]
    pub params: Vec<FnParam<'db>>,
    /// The return type of the function, if any.
    ///
    /// Returned type cannot be a reference, so there is no [`ParamMode`]
    /// alternative for it.
    #[tracked]
    #[returns(copy)]
    pub return_type: Option<Type<'db>>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
}

#[salsa::tracked]
impl<'db> Fn<'db> {
    /// Returns the name of the binding function, e.g.
    /// `__cursed_fn__std_mem_drop`
    #[salsa::tracked(returns(ref))]
    pub fn bindings_name(self, db: &'db dyn Db) -> String {
        let path = self.id(db).path(db);
        format!("__cursed_fn__{}", mangle_path(path))
    }

    /// Returns the name of the function, e.g. `drop` for `std::mem::drop`.
    pub fn name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).path(db).last().expect("fn must have a name")
    }

    /// Returns the fully qualified name of the function, e.g.
    /// `std::mem::drop`.
    #[salsa::tracked(returns(ref))]
    pub fn fqn(self, db: &'db dyn Db) -> String {
        self.id(db).path(db).join("::")
    }
}
