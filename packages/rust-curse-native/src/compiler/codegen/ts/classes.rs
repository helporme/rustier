use crate::compiler::{
    codegen::{
        fragment::{
            FragmentOnce, FragmentPrint, blank, curlies, doubles, frag, fragment, idented, spaced,
        },
        ts::{DeclarationScope, jsdoc::render_jsdoc},
    },
    db::Db,
    ir::decals::{DecalClass, DecalDiscriminator, DecalField, DecalVariant},
    ir::rust::FieldName,
};

#[salsa::tracked]
impl<'db> DecalClass<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render_in_scope_ts(self, db: &'db dyn Db, scope: DeclarationScope) -> String {
        match self {
            DecalClass::Plain(plain) => self.render_plain_ts(db, plain.fields(db).collect(), scope),
            DecalClass::Discriminated(discriminated) => self.render_discriminated_ts(
                db,
                discriminated.variants(db),
                discriminated.discriminator(db),
                scope,
            ),
        }
    }

    fn render_plain_ts(
        self,
        db: &'db dyn Db,
        fields: Vec<DecalField<'db>>,
        scope: DeclarationScope,
    ) -> String {
        let members = fragment(|p| {
            let constructor = self.constructor(db);

            for (index, field) in fields.iter().enumerate() {
                if index > 0 {
                    frag!(@).apply(p);
                }
                field.render_ts(db, true).apply(p);
            }

            let mut rendered_method = false;
            for method in self
                .methods(db)
                .filter(|method| Some(*method) != constructor)
            {
                if !fields.is_empty() || rendered_method {
                    frag!(@).apply(p);
                }
                method.render_in_class_ts(db).apply(p);
                rendered_method = true;
            }

            if !fields.is_empty() || rendered_method {
                frag!(@).apply(p);
            }
            brand().apply(p);
            frag!(@).apply(p);
            if let Some(constructor) = constructor {
                constructor.render_as_constructor_ts(db).apply(p);
            } else {
                private_ctor().apply(p);
            }
        });

        let plain = fragment(|p| {
            if fields.is_empty() {
                frag!({}).apply(p);
            } else {
                curlies(fragment(|p| {
                    for (index, field) in fields.iter().enumerate() {
                        if index > 0 {
                            blank().apply(p);
                        }
                        field.render_ts(db, false).apply(p);
                    }
                }))
                .apply(p);
            }
        });

        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            export class ~{self.name(db)}~{{ {members} }}@@

            export ~{scope.declare_prefix()}namespace ~{self.name(db)}~{{
                export type Plain = ~{plain};
            }}
        )
        .print()
    }

    fn render_discriminated_ts(
        self,
        db: &'db dyn Db,
        variants: &'db [DecalVariant<'db>],
        discriminator: DecalDiscriminator<'db>,
        scope: DeclarationScope,
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

        let static_methods = self
            .methods(db)
            .filter(|method| method.is_static(db))
            .map(|method| method.render_in_object_ts(db))
            .collect::<Vec<_>>();
        let static_members = fragment(move |p| {
            if static_methods.is_empty() {
                frag!({}).apply(p);
            } else {
                curlies(spaced(static_methods)).apply(p);
            }
        });

        let instance_methods = self
            .methods(db)
            .filter(|method| !method.is_static(db))
            .map(|method| method.render_in_object_ts(db))
            .collect::<Vec<_>>();

        // Rust identifiers cannot contain `$`, so this per-enum, module-private
        // brand cannot collide with source declarations or imported Rust types.
        let brand_name = format!("${}Brand", self.name(db));
        let instance_brand_name = brand_name.clone();
        let instance_members = fragment(move |p| {
            if !instance_methods.is_empty() {
                spaced(instance_methods).apply(p);
                blank().apply(p);
            }
            frag!(readonly ~[{instance_brand_name}]: never;).apply(p);
        });

        let declare = scope.declare_prefix();

        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            export type ~{self.name(db)}~ = ~{self.name(db)}.Instance & ~{self.name(db)}.Plain;@@

            export ~{declare}const ~{self.name(db)}:~{static_members};@@

            export ~{declare}namespace ~{self.name(db)}~{{
                {plain}@@

                export interface Instance~{{
                    {instance_members}
                }}
            }}@@

            {declare}const ~{brand_name}: unique symbol;
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
    frag!(private __rust_curse_brand: never;)
}

fn private_ctor() -> impl FragmentOnce {
    frag!(private constructor();)
}

#[cfg(test)]
mod tests {
    use crate::{
        assert_ts_snap,
        compiler::{
            codegen::{fragment::FragmentOnce as _, ts::DeclarationScope},
            ir::{decals::DecalMember, rust::Rust},
        },
    };

    macro_rules! test_class {
        ($name:ident, $class_name:literal, { $($source:tt)* }) => {
            #[test]
            fn $name() {
                assert_ts_snap! {
                    source: { $($source)* },
                    expression: |db, p| {
                        let db = &*db;
                        let root = Rust::get(db)
                            .shim_complete(db)
                            .crates(db)
                            .next()
                            .expect("test crate should be lowered")
                            .root(db)
                            .decals(db);
                        let class = root
                            .members(db)
                            .find_map(|member| match member {
                                DecalMember::Class(class) if class.name(db) == $class_name => {
                                    Some(class)
                                }
                                _ => None,
                            })
                            .expect("test class should be lowered");

                        class.render_in_scope_ts(db, DeclarationScope::Module).apply(p);
                    },
                }
            }
        };
    }

    test_class!(unit_class, "Empty", {
        pub struct Empty;
    });

    test_class!(struct_plain_with_methods, "Sine", {
        pub struct Sine {
            pub amplitude: f64,
            pub frequency: f64,
        }

        impl Sine {
            pub fn new() -> Self {
                Self {
                    amplitude: 1.0,
                    frequency: 1.0,
                }
            }

            pub fn static_fn() -> u32 {
                0
            }

            pub fn method_fn(&self) -> u32 {
                0
            }
        }
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

    test_class!(no_self_associated_function, "Test", {
        #[derive(Clone)]
        pub struct Test;

        impl Test {
            /// Creates a value.
            pub fn create(value: i32) -> Self {
                let _ = value;
                Self
            }
        }
    });

    test_class!(constructor_without_arguments, "Test", {
        #[derive(Clone)]
        pub struct Test;

        impl Test {
            /// Creates a test value.
            pub fn new() -> Self {
                Self
            }
        }
    });

    test_class!(constructor_with_arguments_and_named_return, "Test", {
        #[derive(Clone)]
        pub struct Test;

        impl Test {
            pub fn new(value: i32, enabled: bool) -> Test {
                let _ = (value, enabled);
                Test
            }
        }
    });

    test_class!(owned_self_associated_function, "Test", {
        #[derive(Clone)]
        pub struct Test;

        impl Test {
            pub fn owned(self) -> bool {
                true
            }
        }
    });

    test_class!(shared_self_associated_function, "Test", {
        pub struct Test;

        impl Test {
            pub fn shared(&self, value: i32) -> i32 {
                value
            }
        }
    });

    test_class!(exclusive_self_associated_function, "Test", {
        pub struct Test;

        impl Test {
            pub fn exclusive(&mut self, value: i32) {
                let _ = value;
            }
        }
    });

    test_class!(constructor_associated_function, "Test", {
        pub struct Test;

        impl Test {
            pub fn constructor(&self) -> bool {
                true
            }
        }
    });

    test_class!(prototype_associated_function, "Test", {
        pub struct Test;

        impl Test {
            pub fn prototype() -> bool {
                true
            }
        }
    });

    test_class!(method_shadows_field, "Test", {
        pub struct Test {
            pub value: i32,
        }

        impl Test {
            pub fn value(&self) -> i32 {
                self.value
            }
        }
    });

    test_class!(enum_method_shadows_fields, "Test", {
        pub enum Test {
            First { value: i32 },
            Second { value: i32 },
        }

        impl Test {
            pub fn value(&self) -> i32 {
                0
            }
        }
    });

    test_class!(inherent_enum_methods, "Test", {
        #[derive(Clone)]
        pub enum Test {
            First,
            Second,
        }

        impl Test {
            /// Creates the first variant.
            pub fn first() -> Self {
                Self::First
            }

            pub fn is_first(&self) -> bool {
                matches!(self, Self::First)
            }

            pub fn make_second(&mut self) {
                *self = Self::Second;
            }

            pub fn into_bool(self) -> bool {
                matches!(self, Self::First)
            }
        }
    });

    test_class!(enum_reserved_static_methods, "Wave", {
        pub enum Wave {
            Sine(f32),
            Square(f32, f32),
            Triangle(f32, f32),
        }

        impl Wave {
            /// Creates a sine wave.
            pub fn new() -> Self {
                Self::Sine(1.0)
            }

            pub fn static_fn() -> u32 {
                0
            }

            pub fn r#default() -> Self {
                Self::new()
            }

            pub fn method_fn(&self) -> u32 {
                0
            }
        }
    });

    test_class!(enum_reserved_instance_methods, "Wave", {
        pub enum Wave {
            Sine,
        }

        impl Wave {
            pub fn new(&self) -> bool {
                true
            }

            pub fn constructor(&self) -> bool {
                true
            }

            pub fn r#default(&self) -> bool {
                true
            }
        }
    });

    test_class!(empty_enum, "Never", {
        pub enum Never {}
    });
}
