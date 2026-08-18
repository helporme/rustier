use crate::{
    compiler::{
        codegen::{
            api::Api,
            cast::DirectCastRule,
            fragment::{FragmentOnce, FragmentPrint, Printer, doubles, fragment},
            shim::tethering::{
                ClonedField, FieldExpr, TetheredField, TetheringStruct, TetheringStructField,
                TetheringStructType, render_tethering_struct,
            },
        },
        db::Db,
        ir::{
            rust::{AsItem as _, FieldParent, Shape, ValueType},
            shim::{ShimClass, ShimField, ShimType, ShimValueTypeKind},
        },
    },
    frag,
};

#[salsa::tracked]
impl<'db> ShimClass<'db> {
    #[salsa::tracked(returns(ref))]
    pub(super) fn render(self, db: &'db dyn Db) -> String {
        ClassFragment { class: self, db }.print()
    }
}

struct ClassFragment<'db> {
    class: ShimClass<'db>,
    db: &'db dyn Db,
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
            source_type: self.class.src(self.db).into(),
            fields: tethered_fields,
        };

        render_tethering_struct(self.db, tethering_struct)
    }

    fn render_struct<'a>(&'a self) -> impl FragmentOnce + 'a {
        let api = Api::get(self.db);

        let tethering_type_name =
            ValueType::from(self.class.src(self.db)).tethering_struct_name(self.db);

        let symbol = self.class.ffi(self.db).symbol(self.db);

        frag!(
            {api.header_renamed(self.db, symbol)}
            pub struct ~{symbol}~{{
                src: std::rc::Rc<{tethering_type_name}>
            }}
        )
    }
}

impl<'db> ShimField<'db> {
    fn get_tethered_field(
        &self,
        db: &'db dyn Db,
    ) -> TetheredField<'db, ShimFieldExpr<'_, 'db>, ShimFieldExpr<'_, 'db>> {
        let ref_field_expr = ShimFieldExpr {
            field: self,
            db,
            is_mutable: false,
        };
        let mut_field_expr = ShimFieldExpr {
            field: self,
            db,
            is_mutable: true,
        };

        let src_field = self.src(db);
        if src_field.value_type(db).uses_tethering() {
            TetheredField::TetheringStruct(TetheringStructField {
                name: src_field.mangled_name(db),
                get_ref_expr: ref_field_expr,
                get_mut_expr: mut_field_expr,
                source_type: src_field.value_type(db),
                with_setter: !src_field.is_readonly(db),
            })
        } else {
            TetheredField::Cloned(ClonedField {
                name: src_field.mangled_name(db),
                get_ref_expr: ref_field_expr,
                get_mut_expr: mut_field_expr,
                source_type: src_field.value_type(db),
            })
        }
    }
}

#[derive(Clone, Copy)]
struct ShimFieldExpr<'field, 'db> {
    field: &'field ShimField<'db>,
    db: &'db dyn Db,
    is_mutable: bool,
}

impl<'field, 'db: 'field> FieldExpr for ShimFieldExpr<'field, 'db> {
    fn render(&self, source: &str) -> impl FragmentOnce {
        let Self {
            field,
            db,
            is_mutable,
        } = *self;

        let src_field = field.src(db);

        fragment(move |p| match &src_field.parent(db) {
            FieldParent::Struct(..) => {
                let field_name = src_field.name(db);

                if is_mutable {
                    frag!(__tethering::AccessResult::Ok(&mut ~{source}.{field_name})).apply(p);
                } else {
                    frag!(__tethering::AccessResult::Ok(&{source}.{field_name})).apply(p);
                }
            }
            FieldParent::Variant(variant) => {
                let enum_ = variant.parent(db);
                let field_name = src_field.name(db);
                let reason = frag!(
                    {
                        doubles((
                            "cannot access inactive enum variant field `",
                            enum_.fqn(db),
                            "::",
                            variant.name(db),
                            "::",
                            field_name,
                            "`",
                        ))
                    }
                    .to_string()
                );
                let error = Api::get(db).error(db, reason);
                let enum_name = enum_.fqn(db);
                let variant_name = variant.name(db);

                match variant.shape(db) {
                    Shape::Unit => unreachable!("unit enum variants do not have fields"),
                    Shape::Tuple(fields) => {
                        let index = fields
                            .iter()
                            .position(|candidate| {
                                candidate
                                    .as_ref()
                                    .is_some_and(|candidate| candidate.name(db) == field_name)
                            })
                            .expect("enum field must belong to its variant");
                        let pattern = fragment(move |p| {
                            for _ in 0..index {
                                frag!(_,).apply(p);
                            }
                            frag!(__field, ..).apply(p);
                        });

                        frag!(
                            match ~{source}~{{
                                {enum_name}::{variant_name}({pattern}) => Ok(__field),@
                                _ => Err({error}),
                            }}
                        )
                        .apply(p);
                    }
                    Shape::Plain { .. } => {
                        frag!(
                            match ~{source}~{{
                                {enum_name}::{variant_name}~{{ {field_name}: __field, .. }} => Ok(__field),@
                                _ => Err({error}),
                            }}
                        )
                        .apply(p);
                    }
                }
            }
        })
    }
}

pub struct CastClassShimTypeToTetheringStructType;

impl<'db> DirectCastRule<'db, ShimType<'db>, TetheringStructType<'db>>
    for CastClassShimTypeToTetheringStructType
{
    fn predecessor(self, db: &'db dyn Db, to: TetheringStructType<'db>) -> Option<ShimType<'db>> {
        let shim_type = to.source_type.shim_in(db);
        if matches!(shim_type.deborrow().kind, ShimValueTypeKind::Class(..)) {
            Some(shim_type)
        } else {
            None
        }
    }

    fn render<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: ShimType<'db>,
        _to: TetheringStructType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        frag!({ in_expr }.src)
    }
}

pub struct CastTetheringStructTypeToClassShimType;

impl<'db> DirectCastRule<'db, TetheringStructType<'db>, ShimType<'db>>
    for CastTetheringStructTypeToClassShimType
{
    fn predecessor(self, _db: &'db dyn Db, to: ShimType<'db>) -> Option<TetheringStructType<'db>> {
        if let ShimValueTypeKind::Class(_) = to.deborrow().kind {
            Some(TetheringStructType {
                source_type: to.deborrow().src,
            })
        } else {
            None
        }
    }

    fn render<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: TetheringStructType<'db>,
        to: ShimType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let ShimValueTypeKind::Class(target) = to.deborrow().kind else {
            unreachable!();
        };

        frag!({target.symbol(db)}~{{ src: {in_expr} }})
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        assert_rust_snap,
        compiler::{
            codegen::{fragment::FragmentOnce as _, shim::test::TEST_CRATE_NAME},
            ir::rust::Crate,
        },
    };

    macro_rules! test_class {
        ($name:ident, $($source:tt)*) => {
            #[test]
            fn $name() {
                assert_rust_snap! {
                    source: { $($source)* },
                    expression: |db, printer| {
                        let db = &*db;
                        let root = Crate::by(db, TEST_CRATE_NAME)
                            .expect("test crate should be lowered")
                            .root(db)
                            .shim(db);
                        let [class] = root.classes(db).as_slice() else {
                            panic!("test crate root should contain exactly one shim class");
                        };
                        class.render(db).apply(printer);
                    },
                }
            }
        };
    }

    test_class!(unit_struct, pub struct Test;);
    test_class!(tuple_unit_struct, pub struct Test(););
    test_class!(plain_unit_struct, pub struct Test {});
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

#[cfg(test)]
fn test_value_type<'db>(db: &'db dyn Db, name: &str) -> ValueType<'db> {
    use crate::compiler::{
        codegen::shim::test::TEST_CRATE_NAME,
        ir::rust::{Crate, Member},
    };

    let crate_ = Crate::by(db, TEST_CRATE_NAME).expect("test crate should be lowered");

    crate_
        .root(db)
        .members(db)
        .iter()
        .find_map(|member| match member {
            Member::Struct(struct_) if struct_.name(db) == name => Some((*struct_).into()),
            Member::Enum(enum_) if enum_.name(db) == name => Some((*enum_).into()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("test value type `{name}` should be lowered"))
}

#[cfg(test)]
super::test::cast_snapshot! {
    name: struct_in,
    source: {
        #[derive(Clone)]
        pub struct ExampleStruct;
    },
    from: |db| test_value_type(db, "ExampleStruct").shim_in(db),
    to: |db| crate::compiler::ir::rust::Type::Value(test_value_type(db, "ExampleStruct")),
}

#[cfg(test)]
super::test::cast_snapshot! {
    name: struct_ref_in,
    source: {
        #[derive(Clone)]
        pub struct ExampleStruct;
    },
    from: |db| test_value_type(db, "ExampleStruct").shim_in(db),
    to: |db| test_value_type(db, "ExampleStruct").borrowed(),
}

#[cfg(test)]
super::test::cast_snapshot! {
    name: struct_mut_in,
    source: {
        #[derive(Clone)]
        pub struct ExampleStruct;
    },
    from: |db| test_value_type(db, "ExampleStruct").shim_in(db),
    to: |db| test_value_type(db, "ExampleStruct").borrowed_mut(),
}

#[cfg(test)]
super::test::cast_snapshot! {
    name: struct_out,
    source: {
        #[derive(Clone)]
        pub struct ExampleStruct;
    },
    from: |db| crate::compiler::ir::rust::Type::Value(test_value_type(db, "ExampleStruct")),
    to: |db| ShimType::from(test_value_type(db, "ExampleStruct").shim_out(db)),
}

#[cfg(test)]
super::test::cast_snapshot! {
    name: enum_in,
    source: {
        #[derive(Clone)]
        pub enum ExampleEnum {
            Variant,
        }
    },
    from: |db| test_value_type(db, "ExampleEnum").shim_in(db),
    to: |db| crate::compiler::ir::rust::Type::Value(test_value_type(db, "ExampleEnum")),
}

#[cfg(test)]
super::test::cast_snapshot! {
    name: enum_ref_in,
    source: {
        #[derive(Clone)]
        pub enum ExampleEnum {
            Variant,
        }
    },
    from: |db| test_value_type(db, "ExampleEnum").shim_in(db),
    to: |db| test_value_type(db, "ExampleEnum").borrowed(),
}

#[cfg(test)]
super::test::cast_snapshot! {
    name: enum_mut_in,
    source: {
        #[derive(Clone)]
        pub enum ExampleEnum {
            Variant,
        }
    },
    from: |db| test_value_type(db, "ExampleEnum").shim_in(db),
    to: |db| test_value_type(db, "ExampleEnum").borrowed_mut(),
}

#[cfg(test)]
super::test::cast_snapshot! {
    name: enum_out,
    source: {
        #[derive(Clone)]
        pub enum ExampleEnum {
            Variant,
        }
    },
    from: |db| crate::compiler::ir::rust::Type::Value(test_value_type(db, "ExampleEnum")),
    to: |db| ShimType::from(test_value_type(db, "ExampleEnum").shim_out(db)),
}
