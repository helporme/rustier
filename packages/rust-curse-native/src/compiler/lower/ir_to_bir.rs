use crate::compiler::{
    bir::{
        BindingClass, BindingClassField, BindingFn, BindingFnBody, BindingFnParam, BindingItem,
        Bindings, ExternClass, ExternClassField, ExternClassFieldKind, ExternClassKind, ExternFn,
        ExternFnParam, ExternSymbol, ExternType, ExternValueType, FieldAccess,
    },
    db::Db,
    ir::{Enum, Fn, IntoIrTypeExt, Item, ItemIdExt, Mod, Struct, Type, ValueType},
};

#[salsa::tracked]
impl<'db> Mod<'db> {
    /// Lowers this module and all of its descendants into a flat set of
    /// bindings.
    #[salsa::tracked(returns(ref))]
    pub fn lower_bir(self, db: &'db dyn Db) -> Bindings<'db> {
        let mut items = Vec::new();
        self.lower_items_into(db, &mut items);
        Bindings { items }
    }
}

impl<'db> Mod<'db> {
    fn lower_items_into(self, db: &'db dyn Db, binding_items: &mut Vec<BindingItem<'db>>) {
        let source_items = self.items(db);
        binding_items.reserve(source_items.len());

        for item in source_items.iter().copied() {
            match item {
                Item::Enum(enum_) => {
                    binding_items.push(BindingItem::Class(enum_.lower(db)));
                }
                Item::Struct(struct_) => {
                    binding_items.push(BindingItem::Class(struct_.lower(db)));
                }
                Item::Fn(fn_) => binding_items.push(BindingItem::Fn(fn_.lower(db))),
                Item::Mod(mod_) => mod_.lower_items_into(db, binding_items),
            }
        }
    }
}

impl<'db> Struct<'db> {
    fn lower(self, db: &'db dyn Db) -> BindingClass<'db> {
        let fields = self.fields(db);
        let mut external_fields = Vec::with_capacity(fields.len());
        let mut binding_fields = Vec::with_capacity(fields.len());

        for field in fields.iter().copied() {
            let field_name = field.name(db);
            let source_type = field.type_(db);
            let external = ExternClassField {
                symbol: ExternSymbol::new(db, field_name),
                in_type: Type::Value(source_type).lower_extern_in(db),
                out_type: source_type.lower_extern_out(db),
                is_readonly: field.is_readonly(db),
                kind: ExternClassFieldKind::Plain,
            };

            external_fields.push(external.clone());
            binding_fields.push(BindingClassField {
                external,
                source_type,
                access: FieldAccess::StructAccess {
                    struct_: self,
                    field_name: field_name.to_owned(),
                },
            });
        }

        let external = ExternClass::new(
            db,
            ExternSymbol::new(db, self.symbol(db)),
            ExternClassKind::Plain,
            vec![],
            external_fields,
        );

        BindingClass {
            external,
            source_type: self.into_value_type(db),
            fields: binding_fields,
            fns: vec![],
        }
    }
}

impl<'db> Enum<'db> {
    fn lower(self, db: &'db dyn Db) -> BindingClass<'db> {
        let variants = self.variants(db);
        let field_count = variants
            .iter()
            .map(|variant| variant.fields(db).count())
            .sum();
        let mut external_fields = Vec::with_capacity(field_count);
        let mut binding_fields = Vec::with_capacity(field_count);

        for (discriminator, variant) in variants.iter().copied().enumerate() {
            for field in variant.fields(db) {
                let field_name = field.name(db);
                let source_type = field.type_(db);
                let external = ExternClassField {
                    symbol: ExternSymbol::new(db, field_name.to_owned()),
                    in_type: Type::Value(source_type).lower_extern_in(db),
                    out_type: source_type.lower_extern_out(db),
                    is_readonly: field.is_readonly(db),
                    kind: ExternClassFieldKind::TagDiscriminated {
                        discriminator: discriminator as u32,
                    },
                };

                external_fields.push(external.clone());
                binding_fields.push(BindingClassField {
                    external,
                    source_type,
                    access: FieldAccess::EnumVariantAccess {
                        enum_: self,
                        variant,
                        field_name: field_name.to_owned(),
                    },
                });
            }
        }

        let external = ExternClass::new(
            db,
            ExternSymbol::new(db, self.symbol(db).to_owned()),
            ExternClassKind::TagDiscriminatedUnion {
                tag: ExternSymbol::new(db, "__type".to_owned()),
                names: variants
                    .iter()
                    .map(|variant| variant.name(db).to_owned())
                    .collect(),
            },
            vec![],
            external_fields,
        );

        BindingClass {
            external,
            source_type: self.into_value_type(db),
            fields: binding_fields,
            fns: vec![],
        }
    }
}

impl<'db> Fn<'db> {
    fn lower(self, db: &'db dyn Db) -> BindingFn<'db> {
        let params = self
            .params(db)
            .iter()
            .map(|param| {
                let source_type = param.type_(db);
                let external = ExternFnParam::new(db, source_type.lower_extern_in(db), false);

                BindingFnParam {
                    external,
                    source_type,
                }
            })
            .collect::<Vec<_>>();

        let source_return_type = self.return_type(db);
        let external = ExternFn::new(
            db,
            ExternSymbol::new(db, self.symbol(db)),
            params.iter().map(|param| param.external).collect(),
            source_return_type.map(|type_| type_.lower_extern_out(db)),
        );

        BindingFn {
            external,
            params,
            source_return_type,
            body: BindingFnBody::FnCall(self),
        }
    }
}

impl<'db> Type<'db> {
    pub(crate) fn lower_extern_in(self, db: &'db dyn Db) -> ExternType<'db> {
        let type_ = self.deref();

        if matches!(type_, ValueType::Struct(_) | ValueType::Enum(_)) {
            ExternType::Borrowed(type_.lower_extern(db))
        } else {
            ExternType::Value(type_.lower_extern(db))
        }
    }
}

impl<'db> ValueType<'db> {
    pub(crate) fn lower_extern_out(self, db: &'db dyn Db) -> ExternValueType<'db> {
        self.lower_extern(db)
    }

    fn lower_extern(self, db: &'db dyn Db) -> ExternValueType<'db> {
        match self {
            ValueType::bool => ExternValueType::bool,
            ValueType::char | ValueType::u32 => ExternValueType::u32,
            ValueType::i8 => ExternValueType::i8,
            ValueType::u8 => ExternValueType::u8,
            ValueType::i16 => ExternValueType::i16,
            ValueType::u16 => ExternValueType::u16,
            ValueType::i32 => ExternValueType::i32,
            ValueType::f32 | ValueType::f64 => ExternValueType::f64,
            ValueType::Str | ValueType::String => ExternValueType::String,
            ValueType::Struct(id) => ExternValueType::Class(ExternSymbol::new(db, id.symbol(db))),
            ValueType::Enum(id) => ExternValueType::Class(ExternSymbol::new(db, id.symbol(db))),
        }
    }
}
