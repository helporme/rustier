use crate::codegen::{
    db::Db,
    ir::{mangle::mangle_path, span::Span, type_::Type},
};

#[salsa::interned(debug)]
pub struct FnId<'db> {
    /// Fully qualified path to the fn, e.g. [`std`, `mem`, `drop`]
    #[returns(ref)]
    pub path: Vec<String>,
}

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
    #[tracked]
    #[returns(ref)]
    pub name: String,
    #[tracked]
    #[returns(ref)]
    pub type_: Type,
    #[tracked]
    #[returns(copy)]
    pub mode: ParamMode,
}

/// A function.
#[salsa::tracked(debug)]
pub struct FnDef<'db> {
    pub id: FnId<'db>,
    #[tracked]
    #[returns(ref)]
    pub params: Vec<FnParam<'db>>,
    /// The return type of the function, if any.
    ///
    /// Returned type cannot be a reference, so there is no [`ParamMode`]
    /// alternative for it.
    #[tracked]
    #[returns(copy)]
    pub return_type: Option<Type>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
}

#[salsa::tracked(returns(ref))]
fn mangled_name<'db>(db: &'db dyn Db, id: FnId<'db>) -> String {
    let path = id.path(db);
    format!("__cursed_fn__{}", mangle_path(path))
}

#[salsa::tracked(returns(ref))]
fn fqn<'db>(db: &'db dyn Db, id: FnId<'db>) -> String {
    let path = id.path(db);
    path.join("::")
}

impl<'db> FnDef<'db> {
    /// Returns the mangled name of the function e.g.
    /// `__cursed_fn__std_mem_drop`
    pub fn mangled_name(&self, db: &'db dyn Db) -> &'db str {
        mangled_name(db, self.id(db))
    }

    /// Returns the fully qualified name of the function, e.g.
    /// `std::mem::drop`
    pub fn fqn(&self, db: &'db dyn Db) -> &'db str {
        fqn(db, self.id(db))
    }
}
