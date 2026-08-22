use crate::{
    compiler::{
        codegen::{
            fragment::{
                FragmentOnce, FragmentPrint as _, Printer, comma_list, doubles, fragment, spaced,
                squares,
            },
            js::cast::{Cast, CastRenderer, Renderer},
        },
        db::Db,
        ir::{
            facade::{
                FacadeClass, FacadeClassKind, FacadeFieldDiscriminated, FacadeFieldDiscriminator,
                FacadeFieldPlain, FacadeType,
            },
            rust::FieldName,
        },
    },
    frag,
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
        FacadeClassRenderer {
            class: self,
            db,
            casts,
        }
    }
}

#[derive(Copy, Clone)]
struct FacadeClassRenderer<'db, Casts> {
    class: FacadeClass<'db>,
    db: &'db dyn Db,
    casts: Casts,
}

impl<'db, Casts> FragmentOnce for FacadeClassRenderer<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn apply(self, p: &mut Printer) {
        self.render().apply(p);
    }
}

impl<'db, Casts> FacadeClassRenderer<'db, Casts>
where
    Casts: CastRenderer<'db> + Copy + Clone + 'db,
{
    fn render(self) -> impl FragmentOnce + 'db {
        let name = self.class.name(self.db);

        frag!(
            export class~{name}~{{
                static #_construction_token = Symbol();@
                #_ffi;@
                {self.render_private_declarations()}@

                {self.render_fields()}@@

                constructor(token) {{
                    if (token !== ~{name}.#_construction_token) {{
                        throw new TypeError("rust-backed structs cannot be constructed directly (yet)");
                    }}
                }}@@

                static $from_ffi(ffi) {{
                    const obj = new ~{name}(~{name}.#_construction_token);@
                    obj.#_ffi = ffi;@
                    return obj;
                }}@@

                $ffi() {{
                    return this.#_ffi;
                }}
            }}
        )
    }

    fn render_fields(self) -> impl FragmentOnce + 'db {
        fragment(move |p| match self.class.kind(self.db) {
            FacadeClassKind::Plain { fields } => {
                spaced(
                    fields
                        .iter()
                        .copied()
                        .enumerate()
                        .map(|(index, field)| self.field_renderer(index).render_plain(field)),
                )
                .apply(p);
            }
            FacadeClassKind::DiscriminatedUnion {
                fields,
                discriminator,
            } => {
                spaced(fields.iter().copied().enumerate().map(|(index, field)| {
                    self.field_renderer(index)
                        .render_discriminated(field, *discriminator)
                }))
                .apply(p);

                let discriminator = self
                    .field_renderer(fields.len())
                    .render_discriminator(*discriminator);
                if fields.is_empty() {
                    discriminator.apply(p);
                } else {
                    frag!(@@{discriminator}).apply(p);
                }
            }
        })
    }

    fn render_private_declarations(self) -> impl FragmentOnce + 'db {
        fragment(move |p| match self.class.kind(self.db) {
            FacadeClassKind::Plain { fields } => {
                for (field_index, field) in fields.iter().enumerate() {
                    if matches!(field.type_(self.db), FacadeType::Class(_)) {
                        frag!(#{cache_name(field_index, None)};@).apply(p);
                    }
                }
            }
            FacadeClassKind::DiscriminatedUnion {
                fields,
                discriminator,
            } => {
                for (field_index, field) in fields.iter().enumerate() {
                    for (discriminator, sub) in field.sub(self.db) {
                        if matches!(sub.type_(self.db), FacadeType::Class(_)) {
                            frag!(#{cache_name(field_index, Some(*discriminator))};@).apply(p);
                        }
                    }
                }

                let tags = comma_list(
                    discriminator
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
                frag!(@this.#{cache_name(self.index, None)}~ = ~value;).apply(p);
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
                    this.#{cache_name(self.index, Some(discriminator))}~ = ~value;
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
        let class_name = self.class.name(self.db);

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
            ir::rust::Rust,
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
                            .crates(db)
                            .first()
                            .expect("test crate should be lowered")
                            .root(db)
                            .shim(db)
                            .facade(db);
                        let class = root
                            .classes(db)
                            .iter()
                            .copied()
                            .find(|class| class.name(db) == "Test")
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
}
