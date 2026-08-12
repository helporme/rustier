use crate::compiler::{
    codegen::{cast::DirectCastRule, fragment::FragmentOnce},
    db::Db,
    ir::{
        facade::{FacadeType, FacadeTypeKind},
        ffi::FfiType,
    },
};

// A type used for casting between FFI types and facade types.
#[derive(Clone, Debug, PartialEq, Eq, salsa::Update)]
pub struct FacadeFfiType<'db> {
    pub ffi: FfiType<'db>,
    pub facade: FacadeType<'db>,
}

pub struct CastIndenticalFacadeTypeToFfiTypeRule;

impl<'db> DirectCastRule<'db, FacadeType<'db>, FacadeFfiType<'db>>
    for CastIndenticalFacadeTypeToFfiTypeRule
{
    fn predecessor(self, _db: &'db dyn Db, to: FacadeFfiType<'db>) -> Option<FacadeType<'db>> {
        match to.facade.kind {
            FacadeTypeKind::Number | FacadeTypeKind::String | FacadeTypeKind::Boolean => {
                Some(to.facade)
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

pub struct CastIndenticalFfiTypeToFacadeTypeRule;

impl<'db> DirectCastRule<'db, FacadeFfiType<'db>, FacadeType<'db>>
    for CastIndenticalFfiTypeToFacadeTypeRule
{
    fn predecessor(self, _db: &'db dyn Db, to: FacadeType<'db>) -> Option<FacadeFfiType<'db>> {
        match to.kind {
            FacadeTypeKind::Number | FacadeTypeKind::String | FacadeTypeKind::Boolean => {
                Some(FacadeFfiType {
                    ffi: to.ffi,
                    facade: to,
                })
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
