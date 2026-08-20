use crate::{
    compiler::{
        codegen::{
            fragment::{FragmentOnce, FragmentPrint, blank, curlies, doubles, fragment, idented},
            ts::jsdoc::render_jsdoc,
        },
        db::Db,
        ir::decals::{DecalClass, DecalClassKind, DecalDiscriminator, DecalField, DecalVariant},
        ir::rust::FieldName,
    },
    frag,
};

#[salsa::tracked]
impl<'db> DecalClass<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render_ts(self, db: &'db dyn Db) -> String {
        match self.kind(db) {
            DecalClassKind::Plain { fields } => self.render_plain_ts(db, fields),
            DecalClassKind::DiscriminatedUnion {
                variants,
                discriminator,
            } => self.render_discriminated_ts(db, variants, *discriminator),
        }
    }

    fn render_plain_ts(self, db: &'db dyn Db, fields: &'db [DecalField<'db>]) -> String {
        let fields = fragment(move |p| {
            for (index, field) in fields.iter().enumerate() {
                if index > 0 {
                    frag!(@).apply(p);
                }
                field.render_ts(db, true).apply(p);
            }

            if !fields.is_empty() {
                frag!(@).apply(p);
            }
            brand().apply(p);
        });

        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            export class ~{self.name(db)}~{{ {fields} }}
        )
        .print()
    }

    fn render_discriminated_ts(
        self,
        db: &'db dyn Db,
        variants: &'db [DecalVariant<'db>],
        discriminator: DecalDiscriminator<'db>,
    ) -> String {
        let rendered_variants = fragment(move |p| {
            for (index, variant) in variants.iter().enumerate() {
                if index > 0 {
                    blank().apply(p);
                }
                variant
                    .render_ts(db, discriminator.name(db), variant.name(db))
                    .apply(p);
            }
            frag!(;).apply(p);
        });
        let plain = fragment(move |p| {
            frag!(export type Plain =).apply(p);

            if variants.is_empty() {
                frag!( never;).apply(p);
            } else {
                blank().apply(p);
                idented(rendered_variants).apply(p);
            }
        });

        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            export type ~{self.name(db)}~ = ~{self.name(db)}.Instance & ~{self.name(db)}.Plain;@@

            export namespace ~{self.name(db)}~{{
                {plain}@@

                export class Instance~{{
                    {brand()}
                }}
            }}
        )
        .print()
    }
}

impl<'db> DecalVariant<'db> {
    fn render_ts(
        self,
        db: &'db dyn Db,
        discriminator_name: &'db str,
        tag_name: &'db str,
    ) -> impl FragmentOnce + 'db {
        let properties = fragment(move |p| {
            frag!(readonly ~{discriminator_name}:~{doubles(tag_name)};).apply(p);

            for field in self.fields(db) {
                blank().apply(p);
                field.render_ts(db, false).apply(p);
            }
        });

        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            |~{curlies(properties)}
        )
    }
}

impl<'db> DecalField<'db> {
    fn render_ts(self, db: &'db dyn Db, is_class_field: bool) -> impl FragmentOnce + 'db {
        let visibility = if is_class_field {
            "public "
        } else {
            Default::default()
        };

        let readonly = if self.is_readonly(db) {
            "readonly "
        } else {
            Default::default()
        };

        let name = fragment(move |p| {
            let name = self.name(db);
            match name {
                FieldName::Idx(_) if !is_class_field => ("[", name, "]").apply(p),
                FieldName::Name(value) if !is_class_field && value.parse::<usize>().is_ok() => {
                    ("[", name, "]").apply(p);
                }
                _ => name.apply(p),
            }
        });

        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            {visibility}{readonly}{name}:~{self.type_(db).render_ts(db)};
        )
    }
}

fn brand() -> impl FragmentOnce {
    frag!(
        private __rust_curse_brand: never;@
        private constructor();
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        assert_ts_snap,
        compiler::{codegen::fragment::FragmentOnce as _, ir::rust::Crate, test::TEST_CRATE_NAME},
    };

    macro_rules! test_class {
        ($name:ident, $class_name:literal, { $($source:tt)* }) => {
            #[test]
            fn $name() {
                assert_ts_snap! {
                    source: { $($source)* },
                    expression: |db, p| {
                        let db = &*db;
                        let root = Crate::by(db, TEST_CRATE_NAME)
                            .expect("test crate should be lowered")
                            .root(db)
                            .shim(db)
                            .decals(db);
                        let class = root
                            .classes(db)
                            .iter()
                            .find(|class| class.name(db) == $class_name)
                            .expect("test class should be lowered");

                        class.render_ts(db).apply(p);
                    },
                }
            }
        };
    }

    test_class!(unit_class, "Empty", {
        pub struct Empty;
    });

    test_class!(documented_plain_class, "Record", {
        pub struct Local;

        /// Stores primitive and Rust-backed values.
        pub struct Record {
            /// A writable numeric value.
            pub number: i32,
            /// A value that cannot be replaced from JavaScript.
            pub local: Local,
        }
    });

    test_class!(tuple_class_with_private_field, "Tuple", {
        pub struct Local;

        pub struct Tuple(pub i32, String, pub Local);
    });

    test_class!(documented_discriminated_union, "Shape", {
        pub struct Local;

        /// A shape represented by several layouts.
        pub enum Shape {
            /// An acronym-prefixed request.
            HTTPRequest {
                /// A numeric response code.
                response_code: i32,
            },
            /// A title-cased request with a colliding JavaScript tag.
            HttpRequest { response_code: i32 },
            /// Coordinates represented by tuple fields.
            Point(
                /// A Rust-backed value.
                Local,
                i32,
            ),
            /// A variant without fields.
            Unit,
        }
    });

    test_class!(renamed_discriminator, "Example", {
        pub enum Example {
            Value { r#type: String },
        }
    });
}
