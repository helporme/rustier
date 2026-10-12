use crate::compiler::{
    codegen::fragment::{FragmentOnce, FragmentPrint, doubles, frag, fragment, spaced},
    db::Db,
    ir::facade::{FacadeClassTemplate, FacadeGeneric},
};

#[salsa::tracked]
impl<'db> FacadeClassTemplate<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db) -> String {
        // FIXME: we render "monomorphized" classes as usual _exported_ classes.
        // It means that class like "Option3_5String0" can be imported from a module,
        // even so typescript won't show it.
        let classes = spaced(self.mappings(db).iter().map(|(_, class)| class.render(db)));
        let name = self.name(db);
        let branches = spaced(self.mappings(db).iter().map(|(generics, class)| {
            let arity = generics.len();
            let conditions = fragment(move |p| {
                for (index, generic) in generics.iter().enumerate() {
                    match generic {
                        FacadeGeneric::Symbol(symbol) => {
                            frag!(~&&~ types[{index}] === {doubles(symbol)}).apply(p);
                        }
                        FacadeGeneric::Class(class) => {
                            frag!(~&&~ types[{index}] === {class.import_alias(db)}).apply(p);
                        }
                    }
                }
            });

            frag!(
                if (types.length === {arity}{conditions}) {{
                    return~{class.import_alias(db)};
                }}
            )
        }));

        frag!(
            {classes}@@
            export function~{name}~(...types)~{{
                {branches}@@
                throw new TypeError({doubles(frag!({"Unsupported generic arguments for "}{name}))});
            }}
        )
        .print()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        assert_js_snap,
        compiler::{
            codegen::{fragment::FragmentOnce as _, js::JsCodegenPaths},
            ir::rust::Rust,
        },
    };

    macro_rules! test_template {
        ($name:ident, { $($source:tt)* }) => {
            #[test]
            fn $name() {
                assert_js_snap! {
                    source: { $($source)* },
                    expression: |db, p| {
                        let root = Rust::get(db)
                            .interface_complete(db)
                            .crates(db)
                            .next()
                            .expect("test crate should be lowered")
                            .root(db)
                            .facade(db);
                        let paths = JsCodegenPaths {
                            ffi_js_mod_id: "$rust/unstable!/node".to_owned(),
                        };

                        root.render(db, paths).apply(p);
                    },
                }
            }
        };
    }

    test_template!(primitive_keys, {
        pub struct Wrap<T>(T);

        pub fn consume(
            boolean: &Wrap<bool>,
            character: &Wrap<char>,
            i8_value: &Wrap<i8>,
            u8_value: &Wrap<u8>,
            i16_value: &Wrap<i16>,
            u16_value: &Wrap<u16>,
            i32_value: &Wrap<i32>,
            u32_value: &Wrap<u32>,
            f32_value: &Wrap<f32>,
            f64_value: &Wrap<f64>,
            string: &Wrap<String>,
        ) {
        }
    });

    test_template!(class_and_nested_keys, {
        pub struct Holder(u32);
        pub struct Wrap<T>(T);

        pub fn consume(number: &Wrap<u32>, holder: &Wrap<Holder>, nested: &Wrap<Wrap<u32>>) {}

        pub fn borrowed(value: &Wrap<&u32>) {}
    });

    test_template!(cross_module_keys, {
        pub mod models {
            pub struct Holder(u32);
            pub struct Boxed<T>(T);
            pub enum Status {
                Ready,
            }
        }

        pub mod other {
            pub struct Holder(u32);
        }

        pub struct Holder(u32);
        pub struct Wrap<T>(T);

        pub fn consume(
            holder: &Wrap<models::Holder>,
            other: &Wrap<other::Holder>,
            local: &Wrap<Holder>,
            boxed: &Wrap<models::Boxed<u32>>,
            status: &Wrap<models::Status>,
        ) {
        }
    });

    test_template!(generic_enum, {
        pub struct Holder(u32);
        pub enum Event<T> {
            Value(T),
        }

        pub fn consume(number: &Event<u32>, holder: &Event<Holder>) {}
    });

    test_template!(multiple_generic_arguments, {
        pub struct Pair<T, U>(T, U);

        pub fn consume(first: &Pair<u32, bool>, second: &Pair<bool, u32>) {}
    });

    test_template!(generic_constructor, {
        pub struct Wrap<T>(pub T);

        impl<T> Wrap<T> {
            pub fn new(value: T) -> Self {
                Self(value)
            }
            pub fn identity(value: T) -> T {
                value
            }
        }

        pub fn consume(number: &Wrap<u32>, string: &Wrap<String>, nested: &Wrap<Wrap<u32>>) {}
    });

    test_template!(borrowed_values_excluded, {
        pub struct Wrap<T>(pub T);
        pub enum Event<T> {
            Value(T),
        }

        pub fn consume(value: &Wrap<&u32>, event: &Event<&u32>) {}
    });

    test_template!(unreachable_template, {
        pub struct Wrap<T>(T);
    });
}
