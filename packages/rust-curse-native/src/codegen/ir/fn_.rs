use crate::codegen::{db::Db, ir::item::ItemId, ir::type_::Type};

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

/// A function.
#[salsa::tracked(debug)]
pub struct Fn<'db> {
    pub id: ItemId<'db>,
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
    /// Returns the name of the function, e.g. `drop` for `std::mem::drop`.
    pub fn name(&self, db: &'db dyn Db) -> &'db str {
        self.id(db).name(db)
    }

    /// Returns the fully qualified name of the function, e.g.
    /// `std::mem::drop`.
    pub fn fqn(self, db: &'db dyn Db) -> &'db String {
        self.id(db).fqn(db)
    }

    /// Mangled fully qualified name, e.g. `std__mem__Drop`.
    pub fn mangled_fqn(self, db: &'db dyn Db) -> &'db String {
        self.id(db).mangled_fqn(db)
    }
}
