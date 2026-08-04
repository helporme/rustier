use crate::codegen::{
    bir::{
        BindingClass, BindingClassField, BindingFn, BindingFnBody, BindingFnParam, BindingItem,
        BindingMod, ExternClass, ExternClassField, ExternClassKind, ExternFn, ExternFnParam,
        ExternSymbol, ExternType, ExternValueType, FieldAccess, PlainClassField,
        TagDiscriminatedClassField,
    },
    db::Db,
    ir::{Enum, Fn, IntoIrTypeExt, Item, ItemIdExt, Mod, Struct, Type, ValueType},
};

#[salsa::tracked(returns(ref))]
pub fn lower_mod<'db>(db: &'db dyn Db, mod_: Mod<'db>) -> BindingMod<'db> {
    let items = mod_
        .items(db)
        .iter()
        .filter_map(|item| match item {
            Item::Enum(enum_) => Some(BindingItem::Class(lower_enum(db, *enum_))),
            Item::Struct(struct_) => Some(BindingItem::Class(lower_struct(db, *struct_))),
            Item::Fn(fn_) => Some(BindingItem::Fn(lower_fn(db, *fn_))),
            Item::Mod(_) | Item::CLikeEnum(_) => None,
        })
        .collect();

    BindingMod { items }
}

fn lower_struct<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> BindingClass<'db> {
    let mut external_fields = Vec::new();
    let mut binding_fields = Vec::new();

    for field in struct_.fields(db) {
        let external = PlainClassField {
            symbol: ExternSymbol::new(db, field.name(db).to_owned()),
            type_: lower_value_type(db, field.type_(db)),
            is_readonly: field.is_readonly(db),
        };

        external_fields.push(external.clone());
        binding_fields.push(BindingClassField {
            external: ExternClassField::Plain(external),
            source_type: Type::Value(field.type_(db)),
            access: FieldAccess::StructAccess {
                struct_,
                field_name: field.name(db).to_owned(),
            },
        });
    }

    let external = ExternClass::new(
        db,
        ExternSymbol::new(db, struct_.extern_name(db).to_owned()),
        ExternClassKind::Plain {
            fields: external_fields,
        },
        vec![],
    );

    BindingClass {
        external,
        source_type: Type::Value(struct_.into_ir_type(db)),
        fields: binding_fields,
        fns: vec![],
    }
}

fn lower_enum<'db>(db: &'db dyn Db, enum_: Enum<'db>) -> BindingClass<'db> {
    let mut external_fields = Vec::new();
    let mut binding_fields = Vec::new();

    for (discriminator, variant) in enum_.variants(db).iter().copied().enumerate() {
        for field in variant.fields(db) {
            let external = TagDiscriminatedClassField {
                symbol: ExternSymbol::new(db, field.name(db).to_owned()),
                type_: lower_value_type(db, field.type_(db)),
                is_readonly: field.is_readonly(db),
                discriminator: discriminator as u32,
            };

            external_fields.push(external.clone());
            binding_fields.push(BindingClassField {
                external: ExternClassField::TagDiscriminated(external),
                source_type: Type::Value(field.type_(db)),
                access: FieldAccess::EnumVariantAccess {
                    enum_,
                    variant,
                    field_name: field.name(db).to_owned(),
                },
            });
        }
    }

    let external = ExternClass::new(
        db,
        ExternSymbol::new(db, enum_.extern_name(db).to_owned()),
        ExternClassKind::TagDiscriminatedUnion {
            tag: ExternSymbol::new(db, "__type".to_owned()),
            names: enum_
                .variants(db)
                .iter()
                .map(|variant| variant.name(db).to_owned())
                .collect(),
            fields: external_fields,
        },
        vec![],
    );

    BindingClass {
        external,
        source_type: Type::Value(enum_.into_ir_type(db)),
        fields: binding_fields,
        fns: vec![],
    }
}

fn lower_fn<'db>(db: &'db dyn Db, fn_: Fn<'db>) -> BindingFn<'db> {
    let params = fn_
        .params(db)
        .iter()
        .map(|param| {
            let external = ExternFnParam::new(
                db,
                ExternSymbol::new(db, param.name(db).to_owned()),
                lower_input_type(db, param.type_(db)),
                false,
            );

            BindingFnParam {
                external,
                source_type: param.type_(db),
            }
        })
        .collect::<Vec<_>>();
    let external = ExternFn::new(
        db,
        ExternSymbol::new(db, fn_.extern_name(db).to_owned()),
        params.iter().map(|param| param.external).collect(),
        fn_.return_type(db)
            .map(|type_| lower_extern_value_type(db, type_)),
    );

    BindingFn {
        external,
        params,
        body: BindingFnBody::FnCall(fn_),
    }
}

fn lower_input_type<'db>(db: &'db dyn Db, type_: Type<'db>) -> ExternType<'db> {
    match type_.deref() {
        ValueType::Struct(id) => ExternType::Ref(ExternValueType::Class(ExternSymbol::new(
            db,
            id.extern_name(db).to_owned(),
        ))),
        ValueType::Enum(id) => ExternType::Ref(ExternValueType::Class(ExternSymbol::new(
            db,
            id.extern_name(db).to_owned(),
        ))),
        ValueType::bool
        | ValueType::char
        | ValueType::i8
        | ValueType::u8
        | ValueType::i16
        | ValueType::u16
        | ValueType::i32
        | ValueType::u32
        | ValueType::f32
        | ValueType::f64
        | ValueType::Str
        | ValueType::String => ExternType::Value(ExternValueType::JsAny),
    }
}

fn lower_value_type<'db>(db: &'db dyn Db, type_: ValueType<'db>) -> ExternType<'db> {
    ExternType::Value(lower_extern_value_type(db, type_))
}

fn lower_extern_value_type<'db>(db: &'db dyn Db, type_: ValueType<'db>) -> ExternValueType<'db> {
    match type_ {
        ValueType::bool => ExternValueType::bool,
        ValueType::char | ValueType::u32 => ExternValueType::u32,
        ValueType::i8 => ExternValueType::i8,
        ValueType::u8 => ExternValueType::u8,
        ValueType::i16 => ExternValueType::i16,
        ValueType::u16 => ExternValueType::u16,
        ValueType::i32 => ExternValueType::i32,
        ValueType::f32 | ValueType::f64 => ExternValueType::f64,
        ValueType::Str | ValueType::String => ExternValueType::String,
        ValueType::Struct(id) => {
            ExternValueType::Class(ExternSymbol::new(db, id.extern_name(db).to_owned()))
        }
        ValueType::Enum(id) => {
            ExternValueType::Class(ExternSymbol::new(db, id.extern_name(db).to_owned()))
        }
    }
}
