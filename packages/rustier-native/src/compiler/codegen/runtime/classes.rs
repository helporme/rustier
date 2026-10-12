use crate::compiler::{
    codegen::{
        fragment::{FragmentOnce, FragmentPrint, Printer, doubles, frag, fragment},
        runtime::Platform,
        runtime::tethering::{
            ClonedField, FieldExpr, TetheredField, TetheringStruct, TetheringStructField,
            render_tethering_struct,
        },
    },
    db::Db,
    ir::{
        interface::{InterClass, InterField},
        rust::{FieldParent, TypeKind},
    },
};

#[salsa::tracked]
impl<'db> InterClass<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db, platform: Platform) -> String {
        ClassFragment {
            class: self,
            db,
            platform,
        }
        .print()
    }
}

struct ClassFragment<'db> {
    class: InterClass<'db>,
    db: &'db dyn Db,
    platform: Platform,
}

impl<'db> FragmentOnce for ClassFragment<'db> {
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db> ClassFragment<'db> {
    fn render(&self) -> impl FragmentOnce {
        frag!(
            {self.render_tethering_struct()}@@
            {self.render_struct()}@@
        )
    }

    fn render_tethering_struct<'a>(&'a self) -> impl FragmentOnce + 'a {
        let tethered_fields = self
            .class
            .fields(self.db)
            .iter()
            .map(|field| field.get_tethered_field(self.db));

        let tethering_struct = TetheringStruct {
            source_type: self.class.src_type(self.db),
            fields: tethered_fields,
        };

        render_tethering_struct(self.db, tethering_struct)
    }

    fn render_struct<'a>(&'a self) -> impl FragmentOnce + 'a {
        let platform = self.platform;

        let tethering_type_name = self.class.src_type(self.db).tethering_struct_name(self.db);

        let symbol = self.class.ffi(self.db).symbol(self.db);

        frag!(
            {platform.header_renamed(symbol)}
            pub struct ~{symbol}~{{
                handle: rustier_rt::Handle<{tethering_type_name}>
            }}
        )
    }
}

impl<'db> InterField<'db> {
    fn get_tethered_field(
        &self,
        db: &'db dyn Db,
    ) -> TetheredField<'db, InterFieldExpr<'_, 'db>, InterFieldExpr<'_, 'db>> {
        let ref_field_expr = InterFieldExpr {
            field: self,
            db,
            is_mutable: false,
        };
        let mut_field_expr = InterFieldExpr {
            field: self,
            db,
            is_mutable: true,
        };

        let src_field = self.src(db);
        if src_field._type(db).uses_tethering(db) {
            TetheredField::TetheringStruct(TetheringStructField {
                name: src_field.declaration(db).mangled_name(db),
                get_ref_expr: ref_field_expr,
                get_mut_expr: mut_field_expr,
                source_type: src_field._type(db),
                with_setter: !src_field.is_readonly(db),
            })
        } else {
            TetheredField::Cloned(ClonedField {
                name: src_field.declaration(db).mangled_name(db),
                get_ref_expr: ref_field_expr,
                get_mut_expr: mut_field_expr,
                source_type: src_field._type(db),
            })
        }
    }
}

#[derive(Clone, Copy)]
struct InterFieldExpr<'field, 'db> {
    field: &'field InterField<'db>,
    db: &'db dyn Db,
    is_mutable: bool,
}

impl<'field, 'db: 'field> FieldExpr for InterFieldExpr<'field, 'db> {
    fn render(&self, source: &str) -> impl FragmentOnce {
        let Self {
            field,
            db,
            is_mutable,
        } = *self;

        let src_field = field.src(db);

        fragment(move |p| match &src_field.declaration(db).parent(db) {
            FieldParent::Struct(..) => {
                let field_name = src_field.declaration(db).escaped_name(db);

                if is_mutable {
                    frag!(__tethering::AccessResult::Ok(&mut ~{source}.{field_name})).apply(p);
                } else {
                    frag!(__tethering::AccessResult::Ok(&{source}.{field_name})).apply(p);
                }
            }
            FieldParent::Variant(variant) => {
                let enum_ = src_field.target(db);
                let field_name = src_field.declaration(db).name(db);
                let error = frag!(
                    rustier_rt::EnumError::InactiveField~{{
                        type_name: {doubles(enum_.display(db))},@
                        variant: {doubles(variant.name(db))},@
                        field: {doubles(field_name)},
                    }}.into()
                );

                let TypeKind::Enum(enum_type) = enum_.kind(db) else {
                    unreachable!("variant fields belong to enums")
                };
                let enum_name = enum_type.path(db).fqn(db);
                let variant_name = variant.name(db);
                let field_name = src_field.declaration(db).escaped_name(db);

                frag!(
                    match ~{source}~{{
                        {enum_name}::{variant_name}~{{ {field_name}: __field, .. }} => __tethering::AccessResult::Ok(__field),@
                        _ => __tethering::AccessResult::Err({error}),
                    }}
                )
                .apply(p);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        assert_rust_snap,
        compiler::{codegen::fragment::FragmentOnce as _, ir::rust::Rust},
    };

    macro_rules! test_class {
        ($name:ident, $($source:tt)*) => {
            #[test]
            fn $name() {
                assert_rust_snap! {
                    source: { $($source)* },
                    expression: |db, platform, printer| {
                        let db = &*db;
                        let root = Rust::get(db)
                            .interface_complete(db)
                            .crates(db)
                            .next()
                            .expect("test crate should be lowered")
                            .root(db);

                        let Some(class) = root.classes(db).next() else {
                            panic!("test crate root should contain exactly one interface class");
                        };
                        class.render(db, platform).apply(printer);
                    },
                }
            }
        };
    }

    test_class!(unit_struct, pub struct Test;);
    test_class!(tuple_unit_struct, pub struct Test(););
    test_class!(plain_unit_struct, pub struct Test {});
    test_class!(
        raw_struct_field,
        #[derive(Clone)]
        pub struct Test {
            pub r#type: u32,
        }
    );
    test_class!(
        raw_variant_field,
        #[derive(Clone)]
        pub enum Test {
            Value { r#type: u32 },
        }
    );
    test_class!(tuple_struct_1_field, pub struct Test(u32););
    test_class!(
        plain_struct_1_field,
        pub struct Test {
            pub field: u32,
        }
    );
    test_class!(tuple_struct_2_fields, pub struct Test(pub u32, pub String););
    test_class!(
        plain_struct_2_fields,
        pub struct Test {
            pub field: u32,
            pub field2: String,
        }
    );

    test_class!(
        unit_tuple_variant,
        pub enum Test {
            Tuple(),
        }
    );
    test_class!(
        unit_plain_variant,
        pub enum Test {
            Plain {},
        }
    );
    test_class!(
        tuple_variant,
        pub enum Test {
            Tuple(u32),
        }
    );
    test_class!(
        plain_variant,
        pub enum Test {
            Plain { field: u32 },
        }
    );
    test_class!(
        both_variants,
        pub enum Test {
            Plain { field: String },
            Tuple(i32),
        }
    );
    test_class!(
        clashing_plain_fields,
        pub enum Test {
            Plain1 { field: u32 },
            Plain2 { field: i32 },
        }
    );
    test_class!(
        clashing_tuple_fields,
        pub enum Test {
            Tuple1(u32),
            Tuple2(i32),
        }
    );
    test_class!(
        clashing_both_fields,
        pub enum Test {
            Plain1 { field: u32, field2: i32 },
            Plain2 { field: i32 },
            Tuple1(u32),
            Tuple2(i32),
        }
    );
}
