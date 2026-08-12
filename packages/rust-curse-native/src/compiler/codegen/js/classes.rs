use crate::{
    compiler::{
        codegen::{
            cast::DirectCastRule,
            fragment::{FragmentOnce, Printer},
            js::types::FacadeFfiType,
        },
        db::Db,
        ir::facade::{FacadeFunction, FacadeMod, FacadeType, FacadeTypeKind},
    },
    frag,
};

pub struct CastFacadeClassTypeToFfiClassTypeRule;

impl<'db> DirectCastRule<'db, FacadeType<'db>, FacadeFfiType<'db>>
    for CastFacadeClassTypeToFfiClassTypeRule
{
    fn predecessor(self, _db: &'db dyn Db, to: FacadeFfiType<'db>) -> Option<FacadeType<'db>> {
        match &to.facade.kind {
            FacadeTypeKind::Class(..) => Some(to.facade),
            _ => None,
        }
    }

    fn render<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        from: FacadeType<'db>,
        _to: FacadeFfiType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let FacadeTypeKind::Class(from) = from.kind else {
            unreachable!()
        };

        frag!(
            {in_expr}[{from.name(db)}.$ffi_key]
        )
    }
}

pub struct CastFfiClassTypeToFacadeClassTypeRule;

impl<'db> DirectCastRule<'db, FacadeFfiType<'db>, FacadeType<'db>>
    for CastFfiClassTypeToFacadeClassTypeRule
{
    fn predecessor(self, db: &'db dyn Db, to: FacadeType<'db>) -> Option<FacadeFfiType<'db>> {
        match to.kind {
            FacadeTypeKind::Class(cls) => Some(FacadeFfiType {
                ffi: cls.ffi(db).as_type(),
                facade: to,
            }),
            _ => None,
        }
    }

    fn render<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: FacadeFfiType<'db>,
        to: FacadeType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let FacadeTypeKind::Class(to) = to.kind else {
            unreachable!()
        };

        frag!(
            {to.name(db)}.$from_ffi({in_expr})
        )
    }
}
