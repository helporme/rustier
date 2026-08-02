//! Module for compiling structs into binding structs that can be exposed to
//! JavaScript.

use crate::{
    codegen::{
        bindings::{
            cast_graph::{BindingsCast, BindingsCastCompiler},
            extern_class::{ExternClass, ExternClassField, compile_extern_class},
        },
        db::Db,
        fragment::FragmentPrint,
        ir::{IntoItemId, Struct},
    },
    frag,
};

/// Compiles a Rust struct into its tethered backing struct and public binding
/// wrapper.
#[salsa::tracked(returns(ref))]
pub fn compile_extern_struct<'db>(db: &'db dyn Db, struct_: Struct<'db>) -> String {
    compile_extern_struct_with(db, BindingsCast::Graph, struct_)
}

fn compile_extern_struct_with<'db, CastCompiler>(
    db: &'db dyn Db,
    cast_compiler: CastCompiler,
    struct_: Struct<'db>,
) -> String
where
    CastCompiler: BindingsCastCompiler<'db> + Clone + 'db,
{
    let fields = struct_.fields(db).iter().map(move |field| ExternClassField {
        extern_name: field.name(db),
        type_: field.type_(db),
        is_readonly: field.is_readonly(db),
        get_ref_expr: move |source: &'static str| frag!(__tethering::AccessResult::Ok(&{source}.{field.name(db)})),
        get_mut_expr: move |source: &'static str| frag!(__tethering::AccessResult::Ok(&mut ~{source}.{field.name(db)})),
    });

    let extern_class = ExternClass {
        source_item_id: struct_.into_item_id(db),
        fields,
    };

    compile_extern_class(db, cast_compiler, extern_class).print()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assert_rust_snap,
        codegen::{
            bindings::cast_graph::test::TestCastCompiler, fragment::FragmentOnce,
            rustdoc::struct_::lower_struct,
        },
    };

    macro_rules! test_struct {
        ($name:ident, $($source:tt)*) => {
            #[test]
            fn $name() {
                assert_rust_snap! {
                    source: { $($source)* },
                    expression: |db, p| {
                        let struct_ = lower_struct(db, db.rustdoc_id("Test")).unwrap();
                        compile_extern_struct_with(db, TestCastCompiler, struct_).apply(p);
                    },
                }
            }
        };
    }

    test_struct!(unit_struct, pub struct Test;);
    test_struct!(tuple_unit_struct, pub struct Test(););
    test_struct!(plain_unit_struct, pub struct Test {});
    test_struct!(tuple_struct_1_field, pub struct Test(u32););
    test_struct!(
        plain_struct_1_field,
        pub struct Test {
            pub field: u32,
        }
    );
    test_struct!(tuple_struct_2_fields, pub struct Test(pub u32, pub String););
    test_struct!(
        plain_struct_2_fields,
        pub struct Test {
            pub field: u32,
            pub field2: String,
        }
    );
}
