//! Module for compiling enums into binding structs that can be exposed to
//! JavaScript.

use crate::{
    codegen::{
        api::Api,
        bindings::{
            cast_graph::{BindingsCast, BindingsCastCompiler},
            extern_class::{
                ExternClass, ExternClassField, compile_extern_class, compile_extern_class_fns,
            },
            extern_fn::{ExternFn, ExternFnParam},
        },
        db::Db,
        fragment::{FragmentOnce, FragmentPrint, doubles, fragment},
        ir::{
            enum_::{Enum, EnumVariant},
            fn_::PassingMode,
            item::{IntoItemId as _, ItemIdExt as _},
            shape::{Shape, ShapeField},
            type_::{IrPrimitive, Type},
        },
    },
    frag,
};

#[salsa::tracked]
impl<'db> ShapeField<'db> {
    #[salsa::tracked(returns(ref))]
    // todo: remove pub
    pub fn extern_name(self, db: &'db dyn Db, variant: EnumVariant<'db>) -> String {
        format!("__{}__{}", variant.name(db), self.name(db))
    }
}

/// Compiles a Rust enum into its tethered backing struct and public binding
/// wrapper.
#[salsa::tracked(returns(ref))]
pub fn compile_extern_enum<'db>(db: &'db dyn Db, enum_: Enum<'db>) -> String {
    compile_extern_enum_with(db, BindingsCast::Graph, enum_)
}

fn compile_extern_enum_with<'db, CastCompiler>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    enum_: Enum<'db>,
) -> String
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
{
    let fields = enum_.variants(db).iter().copied().flat_map(move |variant| {
        variant.fields(db).map(move |field| {
            let get_expr = move |source: &'static str| {
                compile_variant_field_access_expr(db, enum_, variant, field, source)
            };

            ExternClassField {
                extern_name: field.extern_name(db, variant),
                type_: field.type_(db),
                is_readonly: field.is_readonly(db),
                get_ref_expr: get_expr,
                get_mut_expr: get_expr,
            }
        })
    });

    let extern_class = ExternClass {
        source_item_id: enum_.into_item_id(db),
        fields,
    };
    let extern_class = compile_extern_class(db, cast_compiler.clone(), extern_class);

    let extern_class_fns = compile_extern_class_fns(
        db,
        cast_compiler,
        enum_.into_item_id(db),
        [compile_type_method(db, enum_)],
    );

    frag!(
        {extern_class}@@
        {extern_class_fns}
    )
    .print()
}

fn compile_variant_field_access_expr<'db>(
    db: &'db dyn Db,
    enum_: Enum<'db>,
    variant: EnumVariant<'db>,
    field: ShapeField<'db>,
    source: &'static str,
) -> impl FragmentOnce + 'db {
    fragment(move |printer| {
        let api = Api::get(db);
        let reason = frag!(
            {
                doubles((
                    "cannot access inactive enum variant field `",
                    enum_.fqn(db),
                    "::",
                    variant.name(db),
                    "::",
                    field.name(db),
                    "`",
                ))
            }
            .to_string()
        );
        let error = api.error(db, reason);
        let variant_name = variant.name(db);

        match variant.shape(db) {
            Shape::Unit => unreachable!(),
            Shape::Tuple(fields) => {
                let index = fields
                    .iter()
                    .position(|candidate| candidate.as_ref() == Some(&field))
                    .expect("enum field must belong to its variant");
                let pattern = fragment(move |p| {
                    for _ in 0..index {
                        frag!(_,).apply(p);
                    }
                    frag!(field, ..).apply(p);
                });

                frag!(
                    match ~{source}~{{
                        {enum_.fqn(db)}::{variant_name}({pattern}) => Ok(field),@
                        _ => Err({error}),
                    }}
                )
                .apply(printer);
            }
            Shape::Plain { .. } => {
                let field_name = field.name(db);
                frag!(
                    match ~{source}~{{
                        {enum_.fqn(db)}::{variant_name}~{{ {field_name}: field, .. }} => Ok(field),@
                        _ => Err({error}),
                    }}
                )
                .apply(printer);
            }
        }
    })
}

fn compile_type_method<'db>(
    db: &'db dyn Db,
    enum_: Enum<'db>,
) -> ExternFn<'db, [ExternFnParam<'db>; 1], impl FragmentOnce> {
    let variants = fragment(move |printer| {
        for (index, variant) in enum_.variants(db).iter().enumerate() {
            let variant_name = variant.name(db);

            match variant.shape(db) {
                Shape::Unit => {
                    frag!({enum_.fqn(db)}::{variant_name}~ => {index}u32,@).apply(printer);
                }
                Shape::Tuple(_) => {
                    frag!({enum_.fqn(db)}::{variant_name}(..) => {index}u32,@).apply(printer);
                }
                Shape::Plain { .. } => {
                    frag!({enum_.fqn(db)}::{variant_name}~{{ .. }} => {index}u32,@).apply(printer);
                }
            }
        }
    });

    let non_exhaustive_fallback = fragment(move |printer| {
        if enum_.is_non_exhaustive(db) {
            frag!(_ => u32::MAX,).apply(printer);
        }
    });

    let expr = frag!(
        match src~{{
            {variants}
            {non_exhaustive_fallback}
        }}
    );

    ExternFn {
        name: "__type",
        params: [ExternFnParam {
            name: "src",
            type_: Type::Enum(enum_.id(db)),
            mode: PassingMode::Ref,
            is_self: true,
        }],
        return_type: Some(Type::Primitive(IrPrimitive::u32)),
        source_expr: expr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assert_rust_snap,
        codegen::{
            bindings::cast_graph::test::TestCastCompiler,
            fragment::FragmentOnce,
            rustdoc::enum_::{LoweredEnum, lower_enum},
        },
    };

    macro_rules! test_enum {
        ($name:ident, $($source:tt)*) => {
            #[test]
            fn $name() {
                assert_rust_snap! {
                    source: { $($source)* },
                    expression: |db, p| {
                        let enum_ = lower_enum(db, db.rustdoc_id("Test")).unwrap();
                        let LoweredEnum::Enum(enum_) = enum_ else {
                            panic!("expected non-c enum");
                        };
                        compile_extern_enum_with(db, TestCastCompiler, enum_).apply(p);
                    },
                }
            }
        };
    }

    test_enum!(
        unit_tuple_variant,
        pub enum Test {
            Tuple(),
        }
    );
    test_enum!(
        unit_plain_variant,
        pub enum Test {
            Plain {},
        }
    );
    test_enum!(
        tuple_variant,
        pub enum Test {
            Tuple(u32),
        }
    );
    test_enum!(
        plain_variant,
        pub enum Test {
            Plain { field: u32 },
        }
    );
    test_enum!(
        both_variants,
        pub enum Test {
            Plain { field: String },
            Tuple(i32),
        }
    );
    test_enum!(
        clashing_plain_fields,
        pub enum Test {
            Plain1 { field: u32 },
            Plain2 { field: i32 },
        }
    );
    test_enum!(
        clashing_tuple_fields,
        pub enum Test {
            Tuple1(u32),
            Tuple2(i32),
        }
    );
    test_enum!(
        clashing_both_fields,
        pub enum Test {
            Plain1 { field: u32, field2: i32 },
            Plain2 { field: i32 },
            Tuple1(u32),
            Tuple2(i32),
        }
    );
}
