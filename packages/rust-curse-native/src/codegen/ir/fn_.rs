use crate::codegen::{
    db::Db,
    ir::{mangle::mangle_path, span::Span, type_::Type, IrId},
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
    pub return_type: Option<Type>,
    #[tracked]
    #[returns(ref)]
    pub docs: Option<String>,
    #[tracked]
    #[returns(ref)]
    pub span: Option<Span>,
}

#[salsa::tracked(returns(ref))]
fn mangled_name<'db>(db: &'db dyn Db, id: IrId<'db>) -> String {
    let path = id.path(db);
    format!("__cursed_fn__{}", mangle_path(path))
}

#[salsa::tracked(returns(ref))]
fn fqn<'db>(db: &'db dyn Db, id: IrId<'db>) -> String {
    id.path(db).join("::")
}

impl<'db> Fn<'db> {
    /// Returns the mangled name of the function e.g.
    /// `__cursed_fn__std_mem_drop`
    pub fn mangled_name(&self, db: &'db dyn Db) -> &'db str {
        mangled_name(db, self.id(db))
    }

    /// Returns the name of the function, e.g. `drop` for `std::mem::drop`.
    pub fn name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).path(db).last().expect("fn must have a name")
    }

    /// Returns the fully qualified name of the function, e.g.
    /// `std::mem::drop`.
    pub fn fqn(&self, db: &'db dyn Db) -> &'db str {
        fqn(db, self.id(db))
    }
}
