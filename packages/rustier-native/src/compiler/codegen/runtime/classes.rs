use crate::compiler::{
    codegen::{
        abi::AbiTransport,
        fragment::{FragmentOnce, FragmentPrint, Printer, frag},
    },
    db::Db,
    ir::runtime::RuntimeClass,
};

#[salsa::tracked]
impl<'db> RuntimeClass<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db, transport: AbiTransport) -> String {
        ClassFragment {
            class: self,
            db,
            transport,
        }
        .print()
    }
}

struct ClassFragment<'db> {
    class: RuntimeClass<'db>,
    db: &'db dyn Db,
    transport: AbiTransport,
}

impl<'db> FragmentOnce for ClassFragment<'db> {
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db> ClassFragment<'db> {
    fn render(&self) -> impl FragmentOnce {
        frag!(
            {self.render_struct()}@@
        )
    }

    fn render_struct<'a>(&'a self) -> impl FragmentOnce + 'a {
        let class = self.class.interface(self.db);
        let tethering_type_name = class.tethering(self.db).tethering_struct_name(self.db);

        class.abi(self.db).render_rust_with_fields(
            self.db,
            self.transport,
            frag!(handle: rustier_rt::Handle<{tethering_type_name}>),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        assert_rust_snap,
        compiler::{
            codegen::fragment::{FragmentOnce as _, frag},
            ir::rust::Rust,
        },
    };

    macro_rules! test_class {
        ($name:ident, $($source:tt)*) => {
            #[test]
            fn $name() {
                assert_rust_snap! {
                    source: { $($source)* },
                    expression: |db, transport, printer| {
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
                        frag!(
                            {class.tethering(db).render(db)}@@
                            {class.runtime(db).render(db, transport)}
                        )
                        .apply(printer);
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
