use crate::compiler::{
    codegen::{cast::DirectCastRule, fragment::FragmentOnce},
    db::Db,
    ir::facade::{FacadeType, FacadeTypeKind},
};

/// A type that represents a FFIType for casting purposes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct FacadeFfiType<'db> {
    pub inner: FacadeType<'db>,
}

pub struct CastIdenticalFacadeTypeToFfiTypeRule;

impl<'db> DirectCastRule<'db, FacadeType<'db>, FacadeFfiType<'db>>
    for CastIdenticalFacadeTypeToFfiTypeRule
{
    fn predecessor(self, _db: &'db dyn Db, to: FacadeFfiType<'db>) -> Option<FacadeType<'db>> {
        match to.inner.kind {
            FacadeTypeKind::Number | FacadeTypeKind::String | FacadeTypeKind::Boolean => {
                Some(to.inner)
            }
            FacadeTypeKind::Class(..) => None,
        }
    }

    fn render<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: FacadeType<'db>,
        _to: FacadeFfiType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        in_expr
    }
}

pub struct CastIdenticalFfiTypeToFacadeTypeRule;

impl<'db> DirectCastRule<'db, FacadeFfiType<'db>, FacadeType<'db>>
    for CastIdenticalFfiTypeToFacadeTypeRule
{
    fn predecessor(self, _db: &'db dyn Db, to: FacadeType<'db>) -> Option<FacadeFfiType<'db>> {
        match to.kind {
            FacadeTypeKind::Number | FacadeTypeKind::String | FacadeTypeKind::Boolean => {
                Some(FacadeFfiType { inner: to })
            }
            FacadeTypeKind::Class(_) => None,
        }
    }

    fn render<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: FacadeFfiType<'db>,
        _to: FacadeType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        in_expr
    }
}
