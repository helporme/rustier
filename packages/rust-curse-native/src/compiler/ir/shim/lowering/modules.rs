use crate::compiler::{
    db::Db,
    diag::DiagAccumulateExt as _,
    ir::{
        rust::{Member, TypeKind},
        shim::{
            ShimClass, ShimClassDiscriminatedTemplate, ShimClassPlainTemplate, ShimClassTemplate,
            ShimFunction, ShimMember, ShimModule,
        },
    },
};

#[salsa::tracked]
impl<'db> ShimModule<'db> {
    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::shim) fn lower_members(
        self,
        db: &'db dyn Db,
    ) -> Vec<ShimMember<'db>> {
        let usages = self.usages(db);

        let Some(members) = usages.module_members(db, self.src(db)) else {
            return vec![];
        };

        let mut lowered = vec![];
        for member in members {
            match *member {
                Member::Struct(src) if !src.generic_params(db).is_empty() => {
                    let template = ShimClassPlainTemplate::new(db, src, usages);

                    lowered.push(ShimMember::Template(ShimClassTemplate::Plain(template)));
                }
                Member::Enum(src) if !src.generic_params(db).is_empty() => {
                    let template = ShimClassDiscriminatedTemplate::new(db, src, usages);

                    lowered.push(ShimMember::Template(ShimClassTemplate::Discriminated(
                        template,
                    )));
                }
                Member::Struct(_) | Member::Enum(_) => lowered.extend(
                    usages
                        .types(db)
                        .filter(|type_| !type_.contains_borrowed_values(db))
                        .filter_map(|type_| match (*member, type_.kind(db)) {
                            (Member::Struct(src), TypeKind::Struct(type_))
                                if type_.def(db) == src =>
                            {
                                Some(ShimClass::Plain(type_.shim(db)))
                            }
                            (Member::Enum(src), TypeKind::Enum(type_)) if type_.def(db) == src => {
                                Some(ShimClass::Discriminated(type_.shim(db)))
                            }
                            _ => None,
                        })
                        .map(ShimMember::Class),
                ),
                Member::Function(function) => {
                    if let Some(callable) = function.as_qualified().as_callible(db) {
                        match callable.lower(db) {
                            Ok(ShimFunction::Free(function)) => {
                                lowered.push(ShimMember::Function(function));
                            }
                            Ok(_) => unreachable!("free functions lower to free shim functions"),
                            Err(error) => (*error).accumulate(db),
                        }
                    }
                }
                Member::Module(module) => {
                    lowered.push(ShimMember::Module(module.shim_reachable_from(db, usages)))
                }
            }
        }

        lowered
    }
}
