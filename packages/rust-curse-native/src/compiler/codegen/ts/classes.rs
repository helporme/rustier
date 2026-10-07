use crate::compiler::{
    codegen::{
        fragment::{
            FragmentOnce, FragmentPrint, Printer, blank, curlies, doubles, frag, fragment, idented,
            spaced,
        },
        ts::{DeclarationScope, jsdoc::render_jsdoc},
    },
    db::Db,
    ir::decals::{
        DecalClass, DecalClassDiscriminated, DecalClassPlain, DecalCoerceFunction, DecalField,
        DecalType, DecalVariant,
    },
    ir::rust::FieldName,
};

#[salsa::tracked]
impl<'db> DecalClass<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render_in_scope_ts(self, db: &'db dyn Db, scope: DeclarationScope) -> String {
        match self {
            DecalClass::Plain(class) => DecalPlainClassFragment { class, db, scope }.print(),
            DecalClass::Discriminated(class) => {
                DecalDiscriminatedClassFragment { class, db, scope }.print()
            }
        }
    }
}

struct DecalPlainClassFragment<'db> {
    class: DecalClassPlain<'db>,
    db: &'db dyn Db,
    scope: DeclarationScope,
}

impl FragmentOnce for DecalPlainClassFragment<'_> {
    fn apply(self, printer: &mut Printer) {
        let Self { class, db, scope } = self;
        let fields: Vec<_> = class.fields(db).collect();
        let members = fragment(|p| {
            let constructor = class.constructor(db);

            for (index, field) in fields.iter().enumerate() {
                if index > 0 {
                    frag!(@).apply(p);
                }
                field.render_accessors_ts(db, "public ").apply(p);
            }

            let mut rendered_method = false;
            for method in class.methods(db) {
                if !fields.is_empty() || rendered_method {
                    frag!(@).apply(p);
                }
                method.render_in_class_ts(db).apply(p);
                rendered_method = true;
            }

            if let Some(coerce) = class.coerce(db) {
                if !fields.is_empty() || rendered_method {
                    blank().apply(p);
                }
                coerce.render_fn_ts(db, true).apply(p);
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

        if class.is_template_instance(db) {
            frag!(
                {scope.declare_prefix()}class ~{class.name(db)}~{{ {members} }}
            )
            .apply(printer);

            return;
        }

        let plain = DecalClass::Plain(class).render_plain_body_ts(db);
        let coerce_type = fragment(|p| {
            if let Some(coerce) = class.coerce(db) {
                frag!(@@).apply(p);
                coerce.render_coerce_ts(db).apply(p);
            }
        });

        frag!(
            {render_jsdoc(class.docs(db).map(String::as_str))}
            export class ~{class.name(db)}~{{ {members} }}@@

            export ~{scope.declare_prefix()}namespace ~{class.name(db)}~{{
                export type Plain = ~{plain};{coerce_type}
            }}
        )
        .apply(printer);
    }
}

struct DecalDiscriminatedClassFragment<'db> {
    class: DecalClassDiscriminated<'db>,
    db: &'db dyn Db,
    scope: DeclarationScope,
}

impl FragmentOnce for DecalDiscriminatedClassFragment<'_> {
    fn apply(self, printer: &mut Printer) {
        let Self { class, db, scope } = self;
        let plain = fragment(move |p| {
            frag!(export type Plain =).apply(p);

            if class.variants(db).is_empty() {
                frag!( never;).apply(p);
            } else {
                blank().apply(p);
                idented(frag!({DecalClass::Discriminated(class).render_plain_body_ts(db)};))
                    .apply(p);
            }
        });

        // Rust identifiers cannot contain `$`, so this per-enum, module-private
        // brand cannot collide with source declarations or imported Rust types.
        let brand_name = || frag!(${class.name(db)}Brand);
        let class_brand_name = || frag!(${class.name(db)}ClassBrand);

        let static_methods = class
            .methods(db)
            .filter(|method| method.is_static(db))
            .map(|method| method.render_in_object_ts(db))
            .chain(
                class
                    .coerce(db)
                    .map(|coerce| coerce.render_fn_ts(db, false)),
            );

        let static_members = curlies(fragment(move |p| {
            let mut static_methods = static_methods.peekable();

            if static_methods.peek().is_some() {
                spaced(static_methods).apply(p);
                blank().apply(p);
            }

            frag!(readonly ~[{class_brand_name()}]: never;).apply(p);
        }));

        let instance_methods = class
            .methods(db)
            .filter(|method| !method.is_static(db))
            .map(|method| method.render_in_object_ts(db))
            .collect::<Vec<_>>();

        let instance_members = fragment(move |p| {
            if !instance_methods.is_empty() {
                spaced(instance_methods).apply(p);
                blank().apply(p);
            }
            frag!(readonly ~[{brand_name()}]: never;).apply(p);
        });

        let declare = scope.declare_prefix();

        if class.is_template_instance(db) {
            frag!(
                type ~{class.name(db)}~ = ~${class.name(db)}Instance & ~(
                    {DecalClass::Discriminated(class).render_plain_body_ts(db)}
                );@@

                {declare}const ~{class.name(db)}:~{static_members};@@

                interface ~${class.name(db)}Instance~{{
                    {instance_members}
                }}@@

                {declare}const ~{brand_name()}: unique symbol;@
                {declare}const ~{class_brand_name()}: unique symbol;
            )
            .apply(printer);

            return;
        }

        let coerce_type = fragment(move |p| {
            if let Some(coerce) = class.coerce(db) {
                frag!(@@).apply(p);
                coerce.render_coerce_ts(db).apply(p);
            }
        });

        frag!(
            {render_jsdoc(class.docs(db).map(String::as_str))}
            export type ~{class.name(db)}~ = ~{class.name(db)}.Instance & ~{class.name(db)}.Plain;@@

            export ~{declare}const ~{class.name(db)}:~{static_members};@@

            export ~{declare}namespace ~{class.name(db)}~{{
                {plain}{coerce_type}@@

                export interface Instance~{{
                    {instance_members}
                }}
            }}@@

            {declare}const ~{brand_name()}: unique symbol;@
            {declare}const ~{class_brand_name()}: unique symbol;
        )
        .apply(printer);
    }
}

impl<'db> DecalCoerceFunction<'db> {
    fn render_fn_ts(self, db: &'db dyn Db, in_class: bool) -> String {
        let static_prefix = in_class.then_some("static ");
        let class = self.class(db).type_name(db);

        frag!(
            {static_prefix}{self.name(db)}(value:~{class.render_ts(db, Some("Coerce"))}):~{class.render_ts(db, None)};
        ).print()
    }

    fn render_coerce_ts(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self.class(db) {
            DecalClass::Plain(_) => {
                frag!(export type Coerce = ~{self.class(db).render_coerce_body_ts(db)};).apply(p);
            }
            DecalClass::Discriminated(_) => {
                frag!(export type Coerce =@{idented(frag!({self.class(db).render_coerce_body_ts(db)};))}).apply(p);
            }
        })
    }
}

impl<'db> DecalClass<'db> {
    pub(super) fn render_plain_body_ts(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            Self::Plain(class) => {
                render_record_fields(
                    class
                        .fields(db)
                        .map(move |field| field.render_property_ts(db, field.get_type(db))),
                )
                .apply(p);
            }
            Self::Discriminated(class) => {
                if class.variants(db).is_empty() {
                    frag!(never).apply(p);
                }

                for (index, variant) in class.variants(db).iter().enumerate() {
                    if index > 0 {
                        blank().apply(p);
                    }

                    variant
                        .render_ts(
                            db,
                            class.discriminator(db).name(db),
                            variant
                                .fields(db)
                                .map(move |field| field.render_accessors_ts(db, "")),
                        )
                        .apply(p);
                }
            }
        })
    }

    pub(super) fn render_coerce_body_ts(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| {
            let Some(coerce) = self.coerce(db) else {
                frag!(never).apply(p);

                return;
            };

            match self {
                Self::Plain(_) => {
                    render_record_fields(
                        coerce
                            .input_fields(db)
                            .into_iter()
                            .map(move |field| field.render_property_ts(db, field.set_type(db))),
                    )
                    .apply(p);
                }
                Self::Discriminated(class) => {
                    if self.is_template_instance(db) {
                        frag!(|~{self.type_name(db).render_ts(db, None)}).apply(p);
                    } else {
                        frag!(| Instance & Plain).apply(p);
                    }

                    for variant in class
                        .variants(db)
                        .iter()
                        .filter(|variant| variant.if_coercible(db))
                    {
                        blank().apply(p);
                        variant
                            .render_ts(
                                db,
                                class.discriminator(db).name(db),
                                variant.fields(db).map(move |field| {
                                    field.render_property_ts(db, field.set_type(db))
                                }),
                            )
                            .apply(p);
                    }
                }
            }
        })
    }
}

fn render_record_fields<'a, Field>(
    fields: impl Iterator<Item = Field> + 'a,
) -> impl FragmentOnce + 'a
where
    Field: FragmentOnce + 'a,
{
    fragment(move |p| {
        let mut fields = fields.peekable();
        if fields.peek().is_none() {
            frag!({}).apply(p);
        } else {
            curlies(fragment(move |p| {
                for (index, field) in fields.enumerate() {
                    if index > 0 {
                        blank().apply(p);
                    }
                    field.apply(p);
                }
            }))
            .apply(p);
        }
    })
}

impl<'db> DecalVariant<'db> {
    fn render_ts<Field>(
        self,
        db: &'db dyn Db,
        discriminator_name: &'db str,
        fields: impl Iterator<Item = Field> + 'db,
    ) -> impl FragmentOnce + 'db
    where
        Field: FragmentOnce + 'db,
    {
        let properties = fragment(move |p| {
            frag!(readonly ~{discriminator_name}:~{doubles(self.name(db))};).apply(p);

            for field in fields {
                blank().apply(p);
                field.apply(p);
            }
        });

        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            |~{curlies(properties)}
        )
    }
}

impl<'db> DecalField<'db> {
    fn render_accessors_ts(
        self,
        db: &'db dyn Db,
        visibility: &'static str,
    ) -> impl FragmentOnce + 'db {
        fragment(move |p| {
            frag!(
                {render_jsdoc(self.docs(db).map(String::as_str))}
                {visibility}get ~{self.render_name_ts(db)}():~{self.get_type(db).render_ts(db)};
            )
            .apply(p);
            if !self.is_readonly(db) {
                frag!(
                    @
                    {visibility}set ~{self.render_name_ts(db)}(value:~{self.set_type(db).render_ts(db)});
                )
                .apply(p);
            }
        })
    }

    fn render_property_ts(self, db: &'db dyn Db, type_: DecalType<'db>) -> impl FragmentOnce + 'db {
        let readonly = self.is_readonly(db).then_some("readonly ");
        frag!(
            {render_jsdoc(self.docs(db).map(String::as_str))}
            {readonly}{self.render_name_ts(db)}:~{type_.render_ts(db)};
        )
    }

    fn render_name_ts(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        let name = self.name(db);
        fragment(move |p| match name {
            FieldName::Idx(_) => frag!([{ name }]).apply(p),
            FieldName::Name(value) if value.parse::<usize>().is_ok() => {
                frag!([{ name }]).apply(p);
            }
            _ => name.apply(p),
        })
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

    test_class!(coerce_with_shadowed_method, "Sine", {
        #[derive(Clone)]
        pub struct Sine {
            /// The initial amplitude.
            pub amplitude: f64,
            pub frequency: f64,
        }

        impl Sine {
            pub fn amplitude(&self) -> f64 {
                self.amplitude
            }
        }
    });

    test_class!(tuple_coerce, "Point", {
        #[derive(Clone)]
        pub struct Point(pub u32, pub u32);
    });

    test_class!(struct_coerce_static_collision, "Point", {
        #[derive(Clone)]
        pub struct Point {
            pub x: u32,
        }

        impl Point {
            pub fn of() -> Self {
                Self { x: 0 }
            }
        }
    });

    test_class!(enum_coerce_subset_and_canonical_names, "Wave", {
        #[derive(Clone)]
        #[non_exhaustive]
        pub enum Wave {
            Unit,
            /// A named variant.
            HTTPRequest {
                r#type: u32,
            },
            HttpRequest(u32),
            #[non_exhaustive]
            Hidden(u32),
        }

        impl Wave {
            pub fn of() -> Self {
                Self::Unit
            }
        }
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

    test_class!(field_shadows_method, "Test", {
        pub struct Test {
            pub value: i32,
        }

        impl Test {
            pub fn value(&self) -> i32 {
                self.value
            }
        }
    });

    test_class!(enum_fields_shadow_method, "Test", {
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
