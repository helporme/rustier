//! Module for compiling a cast functions used to convert or reinterpret
//! values passed from the JavaScript to the Rust types expected by the source
//! code.
//!
//! JavaScript value representation is api-specific, see
//! [`crate::codegen::api`].

use std::collections::HashSet;

use crate::{
    codegen::{
        api::Api,
        db::Db,
        fragment::{FragmentOnce, FragmentPrint, doubles, fragment, spaced},
        ir::type_::{IrPrimitive, IrType},
        rustdoc::mod_::{all_mod_ids, lower_mod_enums, lower_mod_fns, lower_mod_structs},
    },
    frag,
};

/// Compiles all cast helper functions required by public Rust functions,
/// structs, and enums.
#[salsa::tracked(returns(ref))]
pub fn compile_cast_fns(db: &dyn Db) -> String {
    let types = collect_cast_types(db);
    let casted_fns = spaced(types.iter().map(|ty| compile_cast_fn(db, *ty)));
    casted_fns.print()
}

/// Collects cast target types used by public functions, structs, and enums.
fn collect_cast_types<'db>(db: &'db dyn Db) -> Vec<IrType<'db>> {
    let mut seen = HashSet::new();
    let mut types = Vec::new();

    for mod_id in all_mod_ids(db) {
        for fn_ in lower_mod_fns(db, *mod_id) {
            for param in fn_.params(db) {
                collect_cast_type(param.type_(db), &mut seen, &mut types);
            }
        }

        for struct_ in lower_mod_structs(db, *mod_id) {
            for field in struct_.fields(db) {
                collect_cast_type(field.type_(db), &mut seen, &mut types);
            }
        }

        for enum_ in lower_mod_enums(db, *mod_id) {
            for variant in enum_.variants(db) {
                for field in variant.fields(db) {
                    collect_cast_type(field.type_(db), &mut seen, &mut types);
                }
            }
        }
    }

    types
}

/// Collects a cast target type and any wider helper types it depends on.
fn collect_cast_type<'db>(
    type_: IrType<'db>,
    seen: &mut HashSet<IrType<'db>>,
    types: &mut Vec<IrType<'db>>,
) {
    if !seen.insert(type_) {
        return;
    }

    match type_ {
        IrType::Str => {
            collect_cast_type(IrType::String, seen, types);
            return; // no need for both str and string, we generate one cast fn
            // for both
        }
        IrType::Primitive(IrPrimitive::Char) => {
            // char is casted to/from u32 when passing a js boundary
            collect_cast_type(IrType::Primitive(IrPrimitive::u32), seen, types);
        }
        IrType::Primitive(
            IrPrimitive::i8
            | IrPrimitive::i16
            | IrPrimitive::i32
            | IrPrimitive::u8
            | IrPrimitive::u16
            | IrPrimitive::u32
            | IrPrimitive::f32,
        ) => {
            // all "normal" numbers are casted from f64 when passing js border
            collect_cast_type(IrType::Primitive(IrPrimitive::f64), seen, types);
        }
        IrType::Primitive(IrPrimitive::Boolean | IrPrimitive::f64) | IrType::String => {}
        IrType::Struct(_) | IrType::Enum(_) => return,
    }

    types.push(type_);
}

/// Compiles a cast helper function for the given type.
fn compile_cast_fn<'db>(db: &'db dyn Db, type_: IrType<'db>) -> impl FragmentOnce + 'db {
    let api = Api::get(db);

    frag!(
        fn ~{type_.cast_fn_name(db)}(value: ~{api.js_type(db)}) -> Result<{compile_cast_res(type_)}, String> ~{{
            {compile_cast_body(db, type_)}
        }}
    )
}

/// Compiles the successful `Result` type of a cast function.
fn compile_cast_res<'db>(type_: IrType<'db>) -> &'db str {
    match type_ {
        IrType::Primitive(primitive) => primitive.fqn(),
        IrType::Str | IrType::String => "String",
        IrType::Struct(_) | IrType::Enum(_) => unreachable!(),
    }
}

/// Compiles the body of a cast function.
fn compile_cast_body<'db>(db: &'db dyn Db, type_: IrType<'db>) -> impl FragmentOnce + 'db {
    fragment(move |p| {
        let api = Api::get(db);

        match type_ {
            IrType::Primitive(IrPrimitive::Boolean) => {
                api.cast_to_bool_with_string_error(db).apply(p)
            }
            IrType::Primitive(IrPrimitive::Char) => compile_char_cast_body().apply(p),
            IrType::Str | IrType::String => api.cast_to_string_with_string_error(db).apply(p),
            IrType::Primitive(IrPrimitive::i8) => {
                compile_signed_integer_cast_body(IrPrimitive::i8).apply(p)
            }
            IrType::Primitive(IrPrimitive::u8) => {
                compile_unsigned_integer_cast_body(IrPrimitive::u8).apply(p)
            }
            IrType::Primitive(IrPrimitive::i16) => {
                compile_signed_integer_cast_body(IrPrimitive::i16).apply(p)
            }
            IrType::Primitive(IrPrimitive::u16) => {
                compile_unsigned_integer_cast_body(IrPrimitive::u16).apply(p)
            }
            IrType::Primitive(IrPrimitive::i32) => {
                compile_signed_integer_cast_body(IrPrimitive::i32).apply(p)
            }
            IrType::Primitive(IrPrimitive::u32) => {
                compile_unsigned_integer_cast_body(IrPrimitive::u32).apply(p)
            }
            IrType::Primitive(IrPrimitive::f32) => compile_f32_cast_body().apply(p),
            IrType::Primitive(IrPrimitive::f64) => api.cast_to_f64_with_string_error(db).apply(p),
            IrType::Struct(_) | IrType::Enum(_) => unreachable!(),
        }
    })
}

/// Compiles a cast body that converts a JavaScript number into a Rust `char`.
fn compile_char_cast_body() -> impl FragmentOnce {
    let u32_cast_fn = IrPrimitive::u32.cast_fn_name();

    frag!(
        let value = ~{u32_cast_fn}(value).map_err(|_| "cannot cast value to char".to_string())?;
        char::from_u32(value).ok_or_else(|| format!("cannot cast {value} to char"))
    )
}

/// Compiles a cast body that converts a JavaScript number into a Rust `f32`.
fn compile_f32_cast_body() -> impl FragmentOnce {
    let f64_cast_fn = IrPrimitive::f64.cast_fn_name();
    let type_name = IrPrimitive::f32.fqn();

    frag!(
        let type_name = ~{doubles(type_name)};@
        let value = ~{f64_cast_fn}(value).map_err(|_| format!("cannot cast value to {type_name}"))?;@
        Ok(value as ~{type_name})
    )
}

/// Compiles a cast body that converts a JavaScript number into a signed Rust
/// integer.
fn compile_signed_integer_cast_body(primitive: IrPrimitive) -> impl FragmentOnce {
    let f64_cast_fn = IrPrimitive::f64.cast_fn_name();
    let type_name = primitive.fqn();

    frag!(
        let type_name = ~{doubles(type_name)};@
        let value = ~{f64_cast_fn}(value).map_err(|_| format!("cannot cast value to {type_name}"))?;@

        if value.trunc()~ != value || value < ~{type_name}::MIN as f64 || value > ~{type_name}::MAX as f64
        ~{{
            return Err(format!("cannot cast {value} to {type_name}"));
        }}@

        Ok(value as ~{type_name})
    )
}

/// Compiles a cast body that converts a JavaScript number into an unsigned Rust
/// integer.
fn compile_unsigned_integer_cast_body(primitive: IrPrimitive) -> impl FragmentOnce {
    let f64_cast_fn = IrPrimitive::f64.cast_fn_name();
    let type_name = primitive.fqn();

    frag! (
        let type_name = ~{doubles(type_name)};@
        let value = ~{f64_cast_fn}(value).map_err(|_| format!("cannot cast value to {type_name}"))?;@

        if value.trunc()~ != value || value < 0 as f64 || value > ~{type_name}::MAX as f64
        ~{{
            return Err(format!("cannot cast {value} to {type_name}"));
        }}@

        Ok(value as ~{type_name})
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{
        api::{Api, ApiKind, WasmTarget},
        db::DbImpl,
        fragment::FragmentPrint,
        test::{assert_snap, test_db},
    };

    #[test]
    fn collects_no_cast_types_from_function_without_arguments() {
        let db = test_db! {
            pub fn example() {}
        };

        assert_eq!(collect_cast_types(&db), vec![]);
    }

    #[test]
    fn collects_one_cast_type_from_function_with_one_argument() {
        let db = test_db! {
            pub fn example(value: bool) {}
        };

        assert_eq!(
            collect_cast_types(&db),
            vec![IrType::Primitive(IrPrimitive::Boolean)]
        );
    }

    #[test]
    fn collects_two_cast_types_from_function_with_two_different_arguments() {
        let db = test_db! {
            pub fn example(enabled: bool, text: String) {}
        };

        assert_eq!(
            collect_cast_types(&db),
            vec![IrType::Primitive(IrPrimitive::Boolean), IrType::String]
        );
    }

    #[test]
    fn collects_no_cast_types_from_plain_struct_without_fields() {
        let db = test_db! {
            pub struct Example {}
        };

        assert_eq!(collect_cast_types(&db), vec![]);
    }

    #[test]
    fn collects_one_cast_type_from_plain_struct_with_one_field() {
        let db = test_db! {
            pub struct Example {
                pub enabled: bool,
            }
        };

        assert_eq!(
            collect_cast_types(&db),
            vec![IrType::Primitive(IrPrimitive::Boolean)]
        );
    }

    #[test]
    fn collects_two_cast_types_from_plain_struct_with_two_different_fields() {
        let db = test_db! {
            pub struct Example {
                pub enabled: bool,
                pub text: String,
            }
        };

        assert_eq!(
            collect_cast_types(&db),
            vec![IrType::Primitive(IrPrimitive::Boolean), IrType::String]
        );
    }

    #[test]
    fn collects_no_cast_types_from_tuple_struct_without_fields() {
        let db = test_db! {
            pub struct Example();
        };

        assert_eq!(collect_cast_types(&db), vec![]);
    }

    #[test]
    fn collects_one_cast_type_from_tuple_struct_with_one_field() {
        let db = test_db! {
            pub struct Example(pub bool);
        };

        assert_eq!(
            collect_cast_types(&db),
            vec![IrType::Primitive(IrPrimitive::Boolean)]
        );
    }

    #[test]
    fn collects_two_cast_types_from_tuple_struct_with_two_different_fields() {
        let db = test_db! {
            pub struct Example(pub bool, pub String);
        };

        assert_eq!(
            collect_cast_types(&db),
            vec![IrType::Primitive(IrPrimitive::Boolean), IrType::String]
        );
    }

    #[test]
    fn collects_no_cast_types_from_plain_enum_variant_without_fields() {
        let db = test_db! {
            pub enum Example {
                Value {},
            }
        };

        assert_eq!(collect_cast_types(&db), vec![]);
    }

    #[test]
    fn collects_one_cast_type_from_plain_enum_variant_with_one_field() {
        let db = test_db! {
            pub enum Example {
                Value { enabled: bool },
            }
        };

        assert_eq!(
            collect_cast_types(&db),
            vec![IrType::Primitive(IrPrimitive::Boolean)]
        );
    }

    #[test]
    fn collects_two_cast_types_from_plain_enum_variant_with_two_different_fields() {
        let db = test_db! {
            pub enum Example {
                Value { enabled: bool, text: String },
            }
        };

        assert_eq!(
            collect_cast_types(&db),
            vec![IrType::Primitive(IrPrimitive::Boolean), IrType::String]
        );
    }

    #[test]
    fn collects_no_cast_types_from_tuple_enum_variant_without_fields() {
        let db = test_db! {
            pub enum Example {
                Value(),
            }
        };

        assert_eq!(collect_cast_types(&db), vec![]);
    }

    #[test]
    fn collects_one_cast_type_from_tuple_enum_variant_with_one_field() {
        let db = test_db! {
            pub enum Example {
                Value(bool),
            }
        };

        assert_eq!(
            collect_cast_types(&db),
            vec![IrType::Primitive(IrPrimitive::Boolean)]
        );
    }

    #[test]
    fn collects_two_cast_types_from_tuple_enum_variant_with_two_different_fields() {
        let db = test_db! {
            pub enum Example {
                Value(bool, String),
            }
        };

        assert_eq!(
            collect_cast_types(&db),
            vec![IrType::Primitive(IrPrimitive::Boolean), IrType::String]
        );
    }

    #[test]
    fn collects_cast_types_from_function_struct_and_enum() {
        let db = test_db! {
            pub fn example(value: bool) {}

            pub struct Struct {
                pub text: String,
            }

            pub enum Enum {
                Value(f64),
            }
        };

        assert_eq!(
            collect_cast_types(&db),
            vec![
                IrType::Primitive(IrPrimitive::Boolean),
                IrType::String,
                IrType::Primitive(IrPrimitive::f64),
            ]
        );
    }

    #[test]
    fn deduplicates_cast_types_from_function_struct_and_enum() {
        let db = test_db! {
            pub fn example(value: bool) {}

            pub struct Struct {
                pub value: bool,
            }

            pub enum Enum {
                Value(bool),
            }
        };

        assert_eq!(
            collect_cast_types(&db),
            vec![IrType::Primitive(IrPrimitive::Boolean)]
        );
    }

    #[test]
    fn collects_no_cast_types_from_empty_module() {
        let db = test_db! {};

        assert_eq!(collect_cast_types(&db), vec![]);
    }

    macro_rules! cast_fn_snapshot_test {
        ($name:ident, $type_:expr) => {
            #[test]
            fn $name() {
                assert_snap!({
                    let mut db = DbImpl::default();

                    Api::set(&mut db, ApiKind::Node);
                    let node = compile_cast_fn(&db, $type_).print();
                    syn::parse_file(&node)
                        .expect("generated Node cast function must be valid Rust syntax");

                    Api::set(
                        &mut db,
                        ApiKind::Wasm {
                            target: WasmTarget::Web,
                        },
                    );
                    let wasm = compile_cast_fn(&db, $type_).print();
                    syn::parse_file(&wasm)
                        .expect("generated Wasm cast function must be valid Rust syntax");

                    ("// Node\n", node, "\n\n// Wasm\n", wasm).print()
                });
            }
        };
    }

    cast_fn_snapshot_test!(
        compiles_bool_cast_fn,
        IrType::Primitive(IrPrimitive::Boolean)
    );
    cast_fn_snapshot_test!(compiles_char_cast_fn, IrType::Primitive(IrPrimitive::Char));
    cast_fn_snapshot_test!(compiles_i8_cast_fn, IrType::Primitive(IrPrimitive::i8));
    cast_fn_snapshot_test!(compiles_u8_cast_fn, IrType::Primitive(IrPrimitive::u8));
    cast_fn_snapshot_test!(compiles_i16_cast_fn, IrType::Primitive(IrPrimitive::i16));
    cast_fn_snapshot_test!(compiles_u16_cast_fn, IrType::Primitive(IrPrimitive::u16));
    cast_fn_snapshot_test!(compiles_i32_cast_fn, IrType::Primitive(IrPrimitive::i32));
    cast_fn_snapshot_test!(compiles_u32_cast_fn, IrType::Primitive(IrPrimitive::u32));
    cast_fn_snapshot_test!(compiles_f32_cast_fn, IrType::Primitive(IrPrimitive::f32));
    cast_fn_snapshot_test!(compiles_f64_cast_fn, IrType::Primitive(IrPrimitive::f64));
    cast_fn_snapshot_test!(compiles_string_cast_fn, IrType::String);
}
