use crate::compiler::{
    db::Db,
    ir::{
        rust::{
            FieldName, FieldParent, FunctionParent, IntoType, IntoValueType as _, Member, Receiver,
            Type, ValueType,
        },
        shim::{
            ShimClass, ShimClassSource, ShimCrate, ShimField, ShimFunction, ShimFunctionAssociated,
            ShimFunctionEnumDiscriminator, ShimFunctionFree, ShimFunctionGetter,
            ShimFunctionSetter, ShimMod, ShimParam, ShimType,
        },
    },
};

#[salsa::tracked]
impl<'db> ShimCrate<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_mods(self, db: &'db dyn Db) -> Vec<ShimMod<'db>> {
        let mut mods = vec![];
        let mut wave = vec![];

        let root = self.src(db).root(db);
        wave.push(root);
        mods.push(root);

        while let Some(mod_) = wave.pop() {
            for member in mod_.members(db) {
                if let Member::Mod(sub) = member {
                    wave.push(*sub);
                    mods.push(*sub);
                }
            }
        }

        mods.into_iter().map(|mod_| mod_.shim(db)).collect()
    }
}

#[salsa::tracked]
impl<'db> ShimMod<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_classes(self, db: &'db dyn Db) -> Vec<ShimClass<'db>> {
        self.src(db)
            .members(db)
            .iter()
            .filter_map(|member| match member {
                Member::Struct(struct_) => Some(struct_.shim(db)),
                Member::Enum(enum_) => Some(enum_.shim(db)),
                _ => None,
            })
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_fns(self, db: &'db dyn Db) -> Vec<ShimFunction<'db>> {
        let mut fns = vec![];

        for member in self.src(db).members(db) {
            if let Member::Function(function) = member {
                fns.push(ShimFunctionFree::new(db, function).as_fn());
                continue;
            }

            let class = match member {
                Member::Struct(struct_) => Some(struct_.shim(db)),
                Member::Enum(enum_) => Some(enum_.shim(db)),
                _ => None,
            };

            if let Some(class) = class {
                fns.extend(class.fns(db));

                for field in class.fields(db) {
                    fns.push(field.getter(db).as_fn());
                    if let Some(setter) = field.setter(db) {
                        fns.push(setter.as_fn());
                    }
                }
            }
        }

        fns
    }
}

#[salsa::tracked]
impl<'db> ShimClass<'db> {
    pub(super) fn lower_parent(self, db: &'db dyn Db) -> ShimMod<'db> {
        match self.src(db) {
            ShimClassSource::Struct(struct_) => struct_.parent(db).shim(db),
            ShimClassSource::Enum(enum_) => enum_.parent(db).shim(db),
        }
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_symbol(self, db: &'db dyn Db) -> String {
        match self.src(db) {
            ShimClassSource::Struct(struct_) => {
                format!("__struct__{}", struct_.path(db).mangled(db))
            }
            ShimClassSource::Enum(enum_) => {
                format!("__enum__{}", enum_.path(db).mangled(db))
            }
        }
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_fields(self, db: &'db dyn Db) -> Vec<ShimField<'db>> {
        let fields = match self.src(db) {
            ShimClassSource::Struct(struct_) => struct_.fields(db),
            ShimClassSource::Enum(enum_) => enum_.fields(db),
        };
        let methods = self.associated_fns(db);

        fields
            .iter()
            .filter(|field| {
                !methods.iter().any(|method| {
                    method.src(db).receiver(db).is_some()
                        && matches!(
                            field.name(db),
                            FieldName::Name(name) if name == method.src(db).name(db)
                        )
                })
            })
            .map(|field| field.shim(db))
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_associated_fns(self, db: &'db dyn Db) -> Vec<ShimFunctionAssociated<'db>> {
        let impls = match self.src(db) {
            ShimClassSource::Struct(struct_) => struct_.impls(db),
            ShimClassSource::Enum(enum_) => enum_.impls(db),
        };

        impls
            .iter()
            .flat_map(|impl_| impl_.fns(db))
            // JavaScript forbids a static class element named `prototype`,
            // including when the name is computed.
            .filter(|function| function.receiver(db).is_some() || function.name(db) != "prototype")
            .map(|function| ShimFunctionAssociated::new(db, *function))
            .collect()
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_fns(self, db: &'db dyn Db) -> Vec<ShimFunction<'db>> {
        let mut fns = self
            .associated_fns(db)
            .iter()
            .map(|function| function.as_fn())
            .collect::<Vec<_>>();

        if let ShimClassSource::Enum(enum_) = self.src(db) {
            fns.push(ShimFunctionEnumDiscriminator::new(db, enum_).as_fn());
        }

        fns
    }
}

#[salsa::tracked]
impl<'db> ShimFunction<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_params(self, db: &'db dyn Db) -> Vec<ShimParam<'db>> {
        match self {
            ShimFunction::Free(function) => function.params(db).clone(),
            ShimFunction::Associated(function) => function.params(db).clone(),
            ShimFunction::Getter(getter) => getter.params(db).to_vec(),
            ShimFunction::Setter(setter) => setter.params(db).to_vec(),
            ShimFunction::EnumDiscriminator(discriminator) => discriminator.params(db).to_vec(),
        }
    }
}

#[salsa::tracked]
impl<'db> ShimFunctionFree<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__free__{}", self.src(db).path(db).mangled(db))
    }

    pub(super) fn lower_ret_type(self, db: &'db dyn Db) -> Option<ShimType<'db>> {
        self.src(db)
            .ret_type(db)
            .map(|ret| ret.as_type().shim_out(db))
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_params(self, db: &'db dyn Db) -> Vec<ShimParam<'db>> {
        let function = self.as_fn();

        self.src(db)
            .params(db)
            .iter()
            .enumerate()
            .map(|(idx, src)| {
                let src_type = src.type_(db);
                let shim_type = src_type.shim_in(db);
                ShimParam::new(db, function, idx, *src_type, shim_type)
            })
            .collect()
    }
}

#[salsa::tracked]
impl<'db> ShimFunctionAssociated<'db> {
    #[salsa::tracked]
    pub(super) fn lower_self_class(self, db: &'db dyn Db) -> ShimClass<'db> {
        match self.src(db).parent(db) {
            FunctionParent::Impl(impl_) => impl_.target(db).shim(db),
            FunctionParent::Mod(_) => {
                unreachable!("associated shim functions must belong to an impl block")
            }
        }
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__associated__{}", self.src(db).path(db).mangled(db))
    }

    pub(super) fn lower_ret_type(self, db: &'db dyn Db) -> Option<ShimType<'db>> {
        self.src(db)
            .ret_type(db)
            .map(|ret| ret.as_type().shim_out(db))
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_params(self, db: &'db dyn Db) -> Vec<ShimParam<'db>> {
        let function = self.as_fn();
        let mut params = Vec::new();

        if let Some(receiver) = self.src(db).receiver(db) {
            let value_type = self.self_class(db).src(db).into_value_type();
            let src_type = match receiver {
                Receiver::Shared => value_type.borrowed(),
                Receiver::Exclusive => value_type.borrowed_mut(),
                Receiver::Owned => value_type.as_type(),
            };

            params.push(ShimParam::new(
                db,
                function,
                0,
                src_type,
                src_type.shim_in(db),
            ));
        }

        let offset = params.len();
        params.extend(
            self.src(db)
                .params(db)
                .iter()
                .enumerate()
                .map(|(idx, src)| {
                    let src_type = *src.type_(db);
                    ShimParam::new(db, function, idx + offset, src_type, src_type.shim_in(db))
                }),
        );
        params
    }
}

#[salsa::tracked]
impl<'db> ShimFunctionGetter<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_symbol(self, db: &'db dyn Db) -> String {
        let src = self.src(db);
        let parent_name = match src.parent(db) {
            FieldParent::Struct(struct_) => struct_.path(db).mangled(db),
            FieldParent::Variant(variant) => variant.parent(db).path(db).mangled(db),
        };

        format!("__fn__getter__{}__{}", parent_name, src.mangled_name(db))
    }

    pub(super) fn lower_ret_type(self, db: &'db dyn Db) -> ShimType<'db> {
        self.src(db).value_type(db).as_type().shim_out(db)
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_params(self, db: &'db dyn Db) -> [ShimParam<'db>; 1] {
        let field = self.src(db);
        let class = match field.parent(db) {
            FieldParent::Struct(struct_) => struct_.shim(db),
            FieldParent::Variant(variant) => variant.parent(db).shim(db),
        };
        let class_shim_type = ShimType::Class {
            is_borrowed: true,
            inner: class,
        };
        let class_src_type = class.src(db).into_type();

        [ShimParam::new(
            db,
            self.as_fn(),
            0,
            class_src_type,
            class_shim_type,
        )]
    }
}

#[salsa::tracked]
impl<'db> ShimFunctionSetter<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_symbol(self, db: &'db dyn Db) -> String {
        let src = self.src(db);
        let parent_name = match src.parent(db) {
            FieldParent::Struct(struct_) => struct_.path(db).mangled(db),
            FieldParent::Variant(variant) => variant.parent(db).path(db).mangled(db),
        };

        format!("__fn__setter__{}__{}", parent_name, src.mangled_name(db))
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_params(self, db: &'db dyn Db) -> [ShimParam<'db>; 2] {
        let field = self.src(db);
        let class = match field.parent(db) {
            FieldParent::Struct(struct_) => struct_.shim(db),
            FieldParent::Variant(variant) => variant.parent(db).shim(db),
        };
        let class_shim_type = ShimType::Class {
            is_borrowed: true,
            inner: class,
        };
        let class_src_type = class.src(db).into_type();
        let value_src_type = field.value_type(db).as_type();
        let value_shim_type = value_src_type.shim_in(db);
        let function = self.as_fn();

        [
            ShimParam::new(db, function, 0, class_src_type, class_shim_type),
            ShimParam::new(db, function, 1, value_src_type, value_shim_type),
        ]
    }
}

#[salsa::tracked]
impl<'db> ShimFunctionEnumDiscriminator<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_symbol(self, db: &'db dyn Db) -> String {
        format!("__fn__disc__{}", self.src(db).path(db).mangled(db))
    }

    #[salsa::tracked(returns(ref))]
    pub(super) fn lower_params(self, db: &'db dyn Db) -> [ShimParam<'db>; 1] {
        let src_type = Type::from(ValueType::from(self.src(db)));

        [ShimParam::new(
            db,
            self.as_fn(),
            0,
            src_type,
            src_type.shim_in(db),
        )]
    }
}
