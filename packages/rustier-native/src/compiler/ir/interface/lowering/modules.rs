use crate::compiler::{
    db::Db,
    ir::{
        interface::{
            InterClass, InterClassDiscriminatedTemplate, InterClassPlainTemplate,
            InterClassTemplate, InterFunctionFree, InterMember, InterModule,
        },
        rust::{Member, TypeKind},
    },
};

#[salsa::tracked]
impl<'db> InterModule<'db> {
    #[salsa::tracked(returns(ref))]
    pub(in crate::compiler::ir::interface) fn lower_members(
        self,
        db: &'db dyn Db,
    ) -> Vec<InterMember<'db>> {
        let usages = self.usages(db);

        let Some(members) = usages.module_members(db, self.src(db)) else {
            return vec![];
        };

        let mut lowered = vec![];
        for member in members {
            match *member {
                Member::Struct(src) if !src.generic_params(db).is_empty() => {
                    let template = InterClassPlainTemplate::new(db, src, usages);

                    lowered.push(InterMember::Template(InterClassTemplate::Plain(template)));
                }
                Member::Enum(src) if !src.generic_params(db).is_empty() => {
                    let template = InterClassDiscriminatedTemplate::new(db, src, usages);

                    lowered.push(InterMember::Template(InterClassTemplate::Discriminated(
                        template,
                    )));
                }
                Member::Struct(_) | Member::Enum(_) => lowered.extend(
                    usages
                        .types(db)
                        .filter_map(|type_| match (*member, type_.kind(db)) {
                            (Member::Struct(src), TypeKind::Struct(type_))
                                if type_.def(db) == src =>
                            {
                                Some(InterClass::Plain(type_.interface(db)))
                            }
                            (Member::Enum(src), TypeKind::Enum(type_)) if type_.def(db) == src => {
                                Some(InterClass::Discriminated(type_.interface(db)))
                            }
                            _ => None,
                        })
                        .map(InterMember::Class),
                ),
                Member::Function(function) => {
                    if let Some(callable) = function.as_qualified().as_callible(db) {
                        lowered.push(InterMember::Function(InterFunctionFree::new(db, callable)));
                    }
                }
                Member::Module(module) => lowered.push(InterMember::Module(
                    module.interface_reachable_from(db, usages),
                )),
            }
        }

        lowered
    }
}
