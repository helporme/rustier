use crate::compiler::{
    codegen::{
        fragment::{
            FragmentOnce, FragmentPrint as _, Printer, comma_list, doubles, frag, fragment, spaced,
            squares,
        },
        js::cast::{Cast, CastRenderer, Renderer},
    },
    db::Db,
    ir::{
        facade::{
            FacadeClass, FacadeCoerce, FacadeFieldDiscriminated, FacadeFieldDiscriminator,
            FacadeFieldPlain, FacadeParam, FacadeType,
        },
        ffi::FfiFunction,
        rust::FieldName,
    },
};

#[salsa::tracked]
impl<'db> FacadeClass<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db) -> String {
        self.render_with(db, Renderer).print()
    }

    fn render_with<Casts>(self, db: &'db dyn Db, casts: Casts) -> impl FragmentOnce
    where
        Casts: CastRenderer<'db> + Copy + Clone + 'db,
    {
        FacadeClassFragment {
            class: self,
            db,
            casts,
        }
    }
}

#[derive(Copy, Clone)]
struct FacadeClassFragment<'db, Casts> {
    class: FacadeClass<'db>,
    db: &'db dyn Db,
    casts: Casts,
}

impl<'db, Casts> FragmentOnce for FacadeClassFragment<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db, Casts> FacadeClassFragment<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn render(self) -> impl FragmentOnce + 'db {
        let name = self.class.import_alias(self.db);
        let public_name = self.class.name(self.db);

        frag!(
            const~{name}~=~class~{public_name}~{{
                static #_construction_token = Symbol();@
                #_ffi;@
                {self.render_private_declarations()}@

                {self.render_members()}{self.render_coerce()}@@

                {self.render_constructor()}@@

                static $from_ffi(ffi) {{
                    const obj = new ~{name}(~{name}.#_construction_token);@
                    obj.#_ffi = ffi;@
                    return obj;
                }}@@

                $ffi() {{
                    return this.#_ffi;
                }}
            }};@@
            export~{{{name}~as~{public_name}}};
        )
    }

    fn render_members(self) -> impl FragmentOnce + 'db {
        fragment(move |p| {
            self.render_fields().apply(p);

            let methods = self.class.methods(self.db).collect::<Vec<_>>();

            if !methods.is_empty() {
                if self.has_rendered_fields() {
                    frag!(@@).apply(p);
                }
                spaced(
                    methods
                        .into_iter()
                        .map(|method| method.render_with(self.db, self.casts)),
                )
                .apply(p);
            }
        })
    }

    fn render_coerce(self) -> impl FragmentOnce + 'db {
        fragment(move |p| {
            if let Some(coerce) = self.class.coerce(self.db) {
                frag!(@@).apply(p);
                CoerceFragment {
                    coerce,
                    db: self.db,
                    casts: self.casts,
                }
                .apply(p);
            }
        })
    }

    fn render_constructor(self) -> impl FragmentOnce + 'db {
        fragment(move |p| {
            if let Some(constructor) = self.class.constructor(self.db) {
                constructor.render_as_constructor(self.db).apply(p);
            } else {
                let class_name = self.class.import_alias(self.db);
                frag!(
                    constructor(token) {{
                        if (token !== ~{class_name}.#_construction_token) {{
                            throw new TypeError("rust-backed structs cannot be constructed directly");
                        }}
                    }}
                )
                .apply(p);
            }
        })
    }

    fn has_rendered_fields(self) -> bool {
        match self.class {
            FacadeClass::Plain(plain) => plain.fields(self.db).next().is_some(),
            FacadeClass::Discriminated(_) => true,
        }
    }

    fn render_fields(self) -> impl FragmentOnce + 'db {
        fragment(move |p| match self.class {
            FacadeClass::Plain(plain) => {
                spaced(
                    plain
                        .fields(self.db)
                        .enumerate()
                        .map(|(index, field)| self.field_renderer(index).render_plain(field)),
                )
                .apply(p);
            }
            FacadeClass::Discriminated(discriminated) => {
                let fields = discriminated.fields(self.db);
                let discriminator = discriminated.discriminator(self.db);
                spaced(fields.iter().copied().enumerate().map(|(index, field)| {
                    self.field_renderer(index)
                        .render_discriminated(field, discriminator)
                }))
                .apply(p);

                let discriminator = self
                    .field_renderer(fields.len())
                    .render_discriminator(discriminator);
                if fields.is_empty() {
                    discriminator.apply(p);
                } else {
                    frag!(@@{discriminator}).apply(p);
                }
            }
        })
    }

    fn render_private_declarations(self) -> impl FragmentOnce + 'db {
        fragment(move |p| match self.class {
            FacadeClass::Plain(plain) => {
                for (field_index, field) in plain.fields(self.db).enumerate() {
                    if matches!(field.type_(self.db), FacadeType::Class(_)) {
                        frag!(#{cache_name(field_index, None)};@).apply(p);
                    }
                }
            }
            FacadeClass::Discriminated(discriminated) => {
                for (field_index, field) in discriminated.fields(self.db).iter().enumerate() {
                    for (discriminator, sub) in field.sub(self.db) {
                        if matches!(sub.type_(self.db), FacadeType::Class(_)) {
                            frag!(#{cache_name(field_index, Some(*discriminator))};@).apply(p);
                        }
                    }
                }

                let tags = comma_list(
                    discriminated
                        .discriminator(self.db)
                        .variant_names(self.db)
                        .iter()
                        .map(|name| doubles(name.as_str())),
                );
                frag!(static #_types~ = ~[{tags}];@).apply(p);
            }
        })
    }

    fn field_renderer(self, index: usize) -> FacadeFieldRenderer<'db, Casts> {
        FacadeFieldRenderer {
            class: self.class,
            index,
            db: self.db,
            casts: self.casts,
        }
    }
}

#[derive(Copy, Clone)]
struct CoerceFragment<'db, Casts> {
    coerce: FacadeCoerce<'db>,
    db: &'db dyn Db,
    casts: Casts,
}

impl<'db, Casts> FragmentOnce for CoerceFragment<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn apply(self, p: &mut Printer) {
        let db = self.db;
        let name = self.coerce.name(db);
        let class_name = self.coerce.class(db).import_alias(db);
        match self.coerce {
            FacadeCoerce::Plain(coerce) => {
                let body = self.render_body(coerce.ffi(db), coerce.fields(db).collect());
                frag!(static ~{name}(value) {{
                    if (value instanceof ~{class_name}) {{
                        return value;
                    }}@
                    {render_coerce_object_guard()}@
                    {body}@
                }})
                .apply(p);
            }
            FacadeCoerce::Discriminated(coerce) => {
                let class = coerce.class(db);
                let discriminator = class.discriminator(db).name(db);
                let cases = fragment(move |p| {
                    for variant in coerce.coerce_variants(db) {
                        let tag = doubles(&variant.name);
                        let body = self.render_body(variant.ffi, variant.fields.clone());
                        frag!(case ~{tag}: {{ {body}@ }}@).apply(p);
                    }
                });
                let message = doubles(format!(
                    "Unknown or nonconstructible {} discriminator ({}): ",
                    class.name(db),
                    discriminator
                ));
                frag!(static ~{name}(value) {{
                    if (value instanceof ~{class_name}) {{
                        return value;
                    }}@
                    {render_coerce_object_guard()}@
                    switch (value[{doubles(discriminator)}]) {{
                        {cases}
                        default: throw new TypeError({message} + String(value[{doubles(discriminator)}]));
                    }}
                }}).apply(p);
            }
        }
    }
}

impl<'db, Casts> CoerceFragment<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn render_body(
        self,
        ffi: FfiFunction<'db>,
        fields: Vec<(FieldName, FacadeParam<'db>)>,
    ) -> impl FragmentOnce + 'db {
        let db = self.db;
        let class = self.coerce.class(db);
        let args = comma_list((0..fields.len()).map(|i| frag!(__ { i })));
        let symbol = ffi.symbol(db);
        let result = self.casts.render_stmt(
            db,
            Cast {
                from: ffi.ret_type(db).expect("coerce returns its class"),
                to: FacadeType::Class(class),
                in_expr: frag!({ symbol }({ args })),
                out_var_name: "result",
            },
            frag!(return result;),
        );
        let casts = fields.into_iter().zip(ffi.params(db)).enumerate().map(
            |(i, ((name, param), ffi_param))| Cast {
                from: param.type_(db),
                to: ffi_param.type_(db),
                in_expr: fragment(move |p| match name {
                    FieldName::Name(name) => frag!(value[{ doubles(name) }]).apply(p),
                    FieldName::Idx(index) => frag!(value[{ index }]).apply(p),
                }),
                out_var_name: frag!(__ { i }),
            },
        );
        self.casts.render_many(db, casts, result)
    }
}

#[derive(Copy, Clone)]
struct FacadeFieldRenderer<'db, Casts> {
    class: FacadeClass<'db>,
    index: usize,
    db: &'db dyn Db,
    casts: Casts,
}

impl<'db, Casts> FacadeFieldRenderer<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn render_plain(self, field: FacadeFieldPlain<'db>) -> impl FragmentOnce + 'db {
        fragment(move |p| {
            let name = field.name(self.db);
            let accessor = render_field_accessor(name);
            let getter_body = self.render_plain_getter(field);

            frag!(
                get ~{accessor}()~{{
                    {getter_body}
                }}
            )
            .apply(p);

            if field.ffi_setter(self.db).is_some() {
                let accessor = render_field_accessor(name);
                frag!(@@
                    set ~{accessor}(value)~{{
                        {self.render_plain_setter(field)}
                    }}
                )
                .apply(p);
            }
        })
    }

    fn render_plain_getter(self, field: FacadeFieldPlain<'db>) -> impl FragmentOnce + 'db {
        let ffi_getter = field.ffi_getter(self.db);
        let ffi_type = ffi_getter
            .ret_type(self.db)
            .expect("facade field getter should return an FFI value");
        let facade_type = field.type_(self.db);

        fragment(move |p| {
            if matches!(facade_type, FacadeType::Class(_)) {
                let cast = self.casts.render_stmt(
                    self.db,
                    Cast {
                        from: ffi_type,
                        to: facade_type,
                        in_expr: frag!({ ffi_getter.symbol(self.db) }(this.#_ffi)),
                        out_var_name: "out",
                    },
                    frag!(this.#{cache_name(self.index, None)}~ = ~out;),
                );

                frag!(
                    if (this.#{cache_name(self.index, None)}~ === undefined)~{{
                        {cast}
                    }}@
                    return this.#{cache_name(self.index, None)};
                )
                .apply(p);
            } else {
                self.casts
                    .render_stmt(
                        self.db,
                        Cast {
                            from: ffi_type,
                            to: facade_type,
                            in_expr: frag!({ ffi_getter.symbol(self.db) }(this.#_ffi)),
                            out_var_name: "out",
                        },
                        frag!(return out;),
                    )
                    .apply(p);
            }
        })
    }

    fn render_plain_setter(self, field: FacadeFieldPlain<'db>) -> impl FragmentOnce + 'db {
        let ffi_setter = field
            .ffi_setter(self.db)
            .expect("plain field setter should exist");
        let [_, value_param] = ffi_setter.params(self.db).as_slice() else {
            unreachable!("facade field setter should have class and value parameters");
        };
        let facade_type = field.type_(self.db);
        let continuation = fragment(move |p| {
            frag!({ ffi_setter.symbol(self.db) }(this.#_ffi, out);).apply(p);

            if matches!(facade_type, FacadeType::Class(_)) {
                frag!(@this.#{cache_name(self.index, None)}~ = undefined;).apply(p);
            }
        });

        self.casts.render_stmt(
            self.db,
            Cast {
                from: facade_type,
                to: value_param.type_(self.db),
                in_expr: "value",
                out_var_name: "out",
            },
            continuation,
        )
    }

    fn render_discriminated(
        self,
        field: FacadeFieldDiscriminated<'db>,
        discriminator: FacadeFieldDiscriminator<'db>,
    ) -> impl FragmentOnce + 'db {
        fragment(move |p| {
            let accessor = render_field_accessor(field.name(self.db));

            frag!(
                get ~{accessor}()~{{
                    {self.render_discriminated_getter(field, discriminator)}
                }}
            )
            .apply(p);

            if field
                .sub(self.db)
                .values()
                .any(|sub| sub.ffi_setter(self.db).is_some())
            {
                let accessor = render_field_accessor(field.name(self.db));
                frag!(@@
                    set ~{accessor}(value)~{{
                        {self.render_discriminated_setter(field, discriminator)}
                    }}
                )
                .apply(p);
            }
        })
    }

    fn render_discriminated_getter(
        self,
        field: FacadeFieldDiscriminated<'db>,
        discriminator: FacadeFieldDiscriminator<'db>,
    ) -> impl FragmentOnce + 'db {
        let ffi_discriminator = discriminator.ffi_getter(self.db);
        let branches = fragment(move |p| {
            for (discriminator, sub) in field.sub(self.db) {
                frag!(
                    if (type_ === ~{discriminator})~{{
                        {self.render_discriminated_getter_branch(*discriminator, *sub)}
                    }}@
                )
                .apply(p);
            }
        });

        frag!(
            const type_ = ~{ffi_discriminator.symbol(self.db)}(this.#_ffi);@
            {branches}
            {self.render_unavailable_field_error(field)}
        )
    }

    fn render_discriminated_getter_branch(
        self,
        discriminator: u32,
        field: FacadeFieldPlain<'db>,
    ) -> impl FragmentOnce + 'db {
        let ffi_getter = field.ffi_getter(self.db);
        let ffi_type = ffi_getter
            .ret_type(self.db)
            .expect("facade field getter should return an FFI value");
        let facade_type = field.type_(self.db);

        fragment(move |p| {
            if matches!(facade_type, FacadeType::Class(_)) {
                let cast = self.casts.render_stmt(
                    self.db,
                    Cast {
                        from: ffi_type,
                        to: facade_type,
                        in_expr: frag!({ ffi_getter.symbol(self.db) }(this.#_ffi)),
                        out_var_name: "out",
                    },
                    frag!(
                        this.#{cache_name(self.index, Some(discriminator))}~ = ~out;
                    ),
                );

                frag!(
                    if (
                        this.#{cache_name(self.index, Some(discriminator))}~ === undefined
                    )~{{
                        {cast}
                    }}@
                    return this.#{cache_name(self.index, Some(discriminator))};
                )
                .apply(p);
            } else {
                self.casts
                    .render_stmt(
                        self.db,
                        Cast {
                            from: ffi_type,
                            to: facade_type,
                            in_expr: frag!({ ffi_getter.symbol(self.db) }(this.#_ffi)),
                            out_var_name: "out",
                        },
                        frag!(return out;),
                    )
                    .apply(p);
            }
        })
    }

    fn render_discriminated_setter(
        self,
        field: FacadeFieldDiscriminated<'db>,
        discriminator: FacadeFieldDiscriminator<'db>,
    ) -> impl FragmentOnce + 'db {
        let ffi_discriminator = discriminator.ffi_getter(self.db);
        let branches = fragment(move |p| {
            for (discriminator, sub) in field.sub(self.db) {
                if sub.ffi_setter(self.db).is_some() {
                    frag!(
                        if (type_ === ~{discriminator})~{{
                            {self.render_discriminated_setter_branch(*discriminator, *sub)}
                        }}@
                    )
                    .apply(p);
                } else {
                    frag!(
                        if (type_ === ~{discriminator})~{{
                            throw new TypeError("Field is readonly on the active enum variant");
                        }}@
                    )
                    .apply(p);
                }
            }
        });

        frag!(
            const type_ = ~{ffi_discriminator.symbol(self.db)}(this.#_ffi);@
            {branches}
            {self.render_unavailable_field_error(field)}
        )
    }

    fn render_discriminated_setter_branch(
        self,
        discriminator: u32,
        field: FacadeFieldPlain<'db>,
    ) -> impl FragmentOnce + 'db {
        let ffi_setter = field
            .ffi_setter(self.db)
            .expect("discriminated field setter branch should be writable");
        let [_, value_param] = ffi_setter.params(self.db).as_slice() else {
            unreachable!("facade field setter should have class and value parameters");
        };
        let facade_type = field.type_(self.db);
        let continuation = fragment(move |p| {
            frag!({ ffi_setter.symbol(self.db) }(this.#_ffi, out);).apply(p);

            if matches!(facade_type, FacadeType::Class(_)) {
                frag!(@
                    this.#{cache_name(self.index, Some(discriminator))}~ = undefined;
                )
                .apply(p);
            }

            frag!(@return;).apply(p);
        });

        self.casts.render_stmt(
            self.db,
            Cast {
                from: facade_type,
                to: value_param.type_(self.db),
                in_expr: "value",
                out_var_name: "out",
            },
            continuation,
        )
    }

    fn render_discriminator(self, field: FacadeFieldDiscriminator<'db>) -> impl FragmentOnce + 'db {
        let accessor = squares(doubles(field.name(self.db)));
        let ffi_getter = field.ffi_getter(self.db);
        let class_name = self.class.import_alias(self.db);

        frag!(
            get ~{accessor}()~{{
                return ~{class_name}.#_types[{ffi_getter.symbol(self.db)}(this.#_ffi)];
            }}
        )
    }

    fn render_unavailable_field_error(
        self,
        field: FacadeFieldDiscriminated<'db>,
    ) -> impl FragmentOnce + 'db {
        let message = doubles((
            "Field ",
            field.name(self.db).to_string(),
            " is not available on the active ",
            self.class.name(self.db),
            " variant",
        ));

        frag!(throw new TypeError({message});)
    }
}

fn render_coerce_object_guard() -> impl FragmentOnce {
    frag!(
        if (value === null || typeof value !== "object") {{
            throw new TypeError("Object coerce expects a non-null object");
        }}
    )
}

fn render_field_accessor(name: &FieldName) -> impl FragmentOnce + '_ {
    fragment(move |p| match name {
        FieldName::Idx(index) => squares(index.to_string()).apply(p),
        FieldName::Name(name) => squares(doubles(name.as_str())).apply(p),
    })
}

fn cache_name(field_index: usize, discriminator: Option<u32>) -> String {
    match discriminator {
        Some(discriminator) => format!("__field_{field_index}_{discriminator}"),
        None => format!("__field_{field_index}"),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        assert_js_snap,
        compiler::{
            codegen::{fragment::FragmentOnce as _, js::cast::test::TestCastRenderer},
            ir::{facade::FacadeMember, rust::Rust},
        },
    };

    macro_rules! test_class {
        ($name:ident, { $($source:tt)* }) => {
            #[test]
            fn $name() {
                assert_js_snap! {
                    source: { $($source)* },
                    expression: |db, printer| {
                        let db = &*db;
                        let root = Rust::get(db)
                            .shim_complete(db)
                            .crates(db)
                            .next()
                            .expect("test crate should be lowered")
                            .root(db)
                            .facade(db);
                        let class = root
                            .members(db)
                            .find_map(|member| match member {
                                FacadeMember::Class(class) if class.name(db) == "Test" => {
                                    Some(class)
                                }
                                _ => None,
                            })
                            .expect("test crate should contain a `Test` facade class");

                        class.render_with(db, TestCastRenderer).apply(printer);
                    },
                }
            }
        };
    }

    test_class!(unit_struct, {
        pub struct Test;
    });
    test_class!(coerce_unit_struct, {
        #[derive(Clone)]
        pub struct Test;
    });
    test_class!(coerce_tuple_struct, {
        #[derive(Clone)]
        pub struct Test(pub i32, pub String);
    });
    test_class!(coerce_nested_value, {
        #[derive(Clone)]
        pub struct Nested;
        #[derive(Clone)]
        pub struct Test {
            pub nested: Nested,
        }
    });
    test_class!(coerce_static_of_collision, {
        #[derive(Clone)]
        pub struct Test;
        impl Test {
            pub fn of() {}
        }
    });
    test_class!(coerce_instance_of, {
        #[derive(Clone)]
        pub struct Test;
        impl Test {
            pub fn of(&self) {}
        }
    });
    test_class!(coerce_enum_variants, {
        #[derive(Clone)]
        pub enum Test {
            Unit,
            EmptyTuple(),
            EmptyRecord {},
            Tuple(i32, String),
            Record { value: i32 },
        }
    });
    test_class!(coerce_canonical_discriminator, {
        #[derive(Clone)]
        #[allow(non_camel_case_types)]
        pub enum Test {
            http { r#type: i32 },
            Http,
        }
    });
    test_class!(coerce_canonical_ineligible_variant, {
        #[derive(Clone)]
        #[allow(non_camel_case_types)]
        pub enum Test {
            http,
            Http(i128),
        }
    });
    test_class!(tuple_unit_struct, {
        pub struct Test();
    });
    test_class!(plain_unit_struct, {
        pub struct Test {}
    });
    test_class!(tuple_struct_1_field, {
        pub struct Test(pub u32);
    });
    test_class!(tuple_struct_2_fields, {
        pub struct Test(pub u32, pub String);
    });
    test_class!(plain_struct_1_field, {
        pub struct Test {
            pub field: u32,
        }
    });
    test_class!(plain_struct_2_fields, {
        pub struct Test {
            pub first: u32,
            pub second: String,
        }
    });
    test_class!(writable_class_field, {
        #[derive(Clone)]
        pub struct Nested;

        pub struct Test {
            pub nested: Nested,
        }
    });
    test_class!(readonly_class_field, {
        pub struct Nested;

        pub struct Test {
            pub nested: Nested,
        }
    });
    test_class!(unit_variants, {
        pub enum Test {
            First,
            Second,
        }
    });
    test_class!(tuple_variant, {
        pub enum Test {
            Tuple(u32),
        }
    });
    test_class!(plain_variant, {
        pub enum Test {
            Plain { field: u32 },
        }
    });
    test_class!(complex_enum, {
        #[derive(Clone)]
        pub struct Writable;

        pub struct Readonly;

        pub enum Test {
            HTTPRequest {
                value: Writable,
                only_first: u32,
                r#type: u32,
            },
            HttpRequest {
                value: Readonly,
            },
        }
    });
    test_class!(no_self_associated_function, {
        #[derive(Clone)]
        pub struct Test;

        impl Test {
            pub fn create(value: i32) -> Self {
                let _ = value;
                Self
            }
        }
    });
    test_class!(constructor_without_arguments, {
        #[derive(Clone)]
        pub struct Test;

        impl Test {
            pub fn new() -> Self {
                Self
            }
        }
    });
    test_class!(constructor_with_arguments_and_named_return, {
        #[derive(Clone)]
        pub struct Test;

        impl Test {
            pub fn new(value: i32, enabled: bool) -> Test {
                let _ = (value, enabled);
                Test
            }
        }
    });
    test_class!(owned_self_associated_function, {
        #[derive(Clone)]
        pub struct Test;

        impl Test {
            pub fn owned(self) -> bool {
                true
            }
        }
    });
    test_class!(shared_self_associated_function, {
        pub struct Test;

        impl Test {
            pub fn shared(&self, value: i32) -> i32 {
                value
            }
        }
    });
    test_class!(exclusive_self_associated_function, {
        pub struct Test;

        impl Test {
            pub fn exclusive(&mut self, value: i32) {
                let _ = value;
            }
        }
    });
    test_class!(constructor_associated_function, {
        pub struct Test;

        impl Test {
            pub fn constructor(&self) -> bool {
                true
            }
        }
    });
    test_class!(prototype_associated_function, {
        pub struct Test;

        impl Test {
            pub fn prototype() -> bool {
                true
            }
        }
    });
    test_class!(field_shadows_method, {
        pub struct Test {
            pub value: i32,
        }

        impl Test {
            pub fn value(&self) -> i32 {
                self.value
            }
        }
    });
    test_class!(enum_fields_shadow_method, {
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
    test_class!(inherent_enum_methods, {
        #[derive(Clone)]
        pub enum Test {
            First,
            Second,
        }

        impl Test {
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
}
