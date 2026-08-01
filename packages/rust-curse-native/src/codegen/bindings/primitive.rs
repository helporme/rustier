use crate::{
    codegen::{
        api::Api,
        bindings::bir::{BirBoundaryType, BirSourceType},
        cast::{CastContinuation, DirectCastRule, IndirectCastRule},
        db::Db,
        fragment::{FragmentOnce, doubles},
    },
    frag,
};

/// Casts an owned boundary `f64` to any supported signed integer type.
pub struct CastF64ToSignedIntRule;

impl<'db> DirectCastRule<'db, BirSourceType<'db>, BirSourceType<'db>> for CastF64ToSignedIntRule {
    fn predecessor(self, _db: &'db dyn Db, to: BirSourceType<'db>) -> Option<BirSourceType<'db>> {
        matches!(
            to,
            BirSourceType::i8 | BirSourceType::i16 | BirSourceType::i32
        )
        .then_some(BirSourceType::f64)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: BirSourceType<'db>,
        to: BirSourceType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let type_name = match to {
            BirSourceType::i8 => "i8",
            BirSourceType::i16 => "i16",
            BirSourceType::i32 => "i32",
            _ => unreachable!("signed integer cast requires a signed integer target"),
        };
        let api = Api::get(db);

        frag!({{
            let type_name = ~{doubles(type_name)};@
            let value = ~{in_expr};@

            if value.trunc()~ != value || value < ~{type_name}::MIN as f64 || value > ~{type_name}::MAX as f64 ~{{
                return Err({
                    api.error(db, frag!(
                        format!("cannot cast {value} to {type_name}")
                    ))
                });
            }}@

            value as ~{type_name}
        }})
    }
}

/// Casts an owned boundary `f64` to any supported unsigned integer type.
pub struct CastF64ToUnsignedIntRule;

impl<'db> DirectCastRule<'db, BirSourceType<'db>, BirSourceType<'db>> for CastF64ToUnsignedIntRule {
    fn predecessor(self, _db: &'db dyn Db, to: BirSourceType<'db>) -> Option<BirSourceType<'db>> {
        matches!(
            to,
            BirSourceType::u8 | BirSourceType::u16 | BirSourceType::u32
        )
        .then_some(BirSourceType::f64)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: BirSourceType<'db>,
        to: BirSourceType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let type_name = match to {
            BirSourceType::u8 => "u8",
            BirSourceType::u16 => "u16",
            BirSourceType::u32 => "u32",
            _ => unreachable!("unsigned integer cast requires an unsigned integer target"),
        };
        let api = Api::get(db);

        frag!({{
            let type_name = ~{doubles(type_name)};@
            let value = ~{in_expr};@

            if value.trunc()~ != value || value < 0 as f64 || value > ~{type_name}::MAX as f64 ~{{
                return Err({
                    api.error(db, frag!(
                        format!("cannot cast {value} to {type_name}")
                    ))
                });
            }}@

            value as ~{type_name}
        }})
    }
}

/// Casts an owned boundary `u32` to a `char`.
pub struct CastU32ToCharRule;

impl<'db> DirectCastRule<'db, BirSourceType<'db>, BirSourceType<'db>> for CastU32ToCharRule {
    fn predecessor(self, _db: &'db dyn Db, to: BirSourceType<'db>) -> Option<BirSourceType<'db>> {
        (to == BirSourceType::char).then_some(BirSourceType::u32)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: BirSourceType<'db>,
        _to: BirSourceType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let api = Api::get(db);
        let error_expr = api.error(db, frag!(format!("cannot cast {value} to char")));

        frag!({{
            let value = ~{in_expr};@
            char::from_u32(value).ok_or_else(||~{error_expr})?
        }})
    }
}

/// Casts an owned boundary `f64` to an `f32`.
pub struct CastF64ToF32Rule;

impl<'db> DirectCastRule<'db, BirSourceType<'db>, BirSourceType<'db>> for CastF64ToF32Rule {
    fn predecessor(self, _db: &'db dyn Db, to: BirSourceType<'db>) -> Option<BirSourceType<'db>> {
        (to == BirSourceType::f32).then_some(BirSourceType::f64)
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: BirSourceType<'db>,
        _to: BirSourceType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        frag!(({in_expr})~as f32)
    }
}

/// Casts an owned source `char` to a boundary `u32`.
pub struct CastCharToU32BoundaryRule;

impl<'db> DirectCastRule<'db, BirSourceType<'db>, BirBoundaryType<'db>>
    for CastCharToU32BoundaryRule
{
    fn predecessor(self, _db: &'db dyn Db, to: BirBoundaryType<'db>) -> Option<BirSourceType<'db>> {
        (to == BirBoundaryType::u32).then_some(BirSourceType::char)
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: BirSourceType<'db>,
        _to: BirBoundaryType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        frag!(({in_expr})~as u32)
    }
}

/// Casts an owned source `f32` to a boundary `f64`.
pub struct CastF32ToF64BoundaryRule;

impl<'db> DirectCastRule<'db, BirSourceType<'db>, BirBoundaryType<'db>>
    for CastF32ToF64BoundaryRule
{
    fn predecessor(self, _db: &'db dyn Db, to: BirBoundaryType<'db>) -> Option<BirSourceType<'db>> {
        (to == BirBoundaryType::f64).then_some(BirSourceType::f32)
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: BirSourceType<'db>,
        _to: BirBoundaryType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        frag!(({in_expr})~as f64)
    }
}

/// Passes an owned primitive source value to its identical boundary type.
pub struct CastPrimitiveToSameBoundaryRule;

impl<'db> DirectCastRule<'db, BirSourceType<'db>, BirBoundaryType<'db>>
    for CastPrimitiveToSameBoundaryRule
{
    fn predecessor(self, _db: &'db dyn Db, to: BirBoundaryType<'db>) -> Option<BirSourceType<'db>> {
        match to {
            BirBoundaryType::String => Some(BirSourceType::String),
            BirBoundaryType::bool => Some(BirSourceType::bool),
            BirBoundaryType::i8 => Some(BirSourceType::i8),
            BirBoundaryType::u8 => Some(BirSourceType::u8),
            BirBoundaryType::i16 => Some(BirSourceType::i16),
            BirBoundaryType::u16 => Some(BirSourceType::u16),
            BirBoundaryType::i32 => Some(BirSourceType::i32),
            BirBoundaryType::u32 => Some(BirSourceType::u32),
            BirBoundaryType::f64 => Some(BirSourceType::f64),
            BirBoundaryType::JsAny | BirBoundaryType::Item { .. } | BirBoundaryType::Ref { .. } => {
                None
            }
        }
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: BirSourceType<'db>,
        _to: BirBoundaryType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        in_expr
    }
}

/// Casts an API-specific JavaScript boundary value to an owned `f64`.
pub struct CastJsAnyToF64Rule;

impl<'db> DirectCastRule<'db, BirBoundaryType<'db>, BirSourceType<'db>> for CastJsAnyToF64Rule {
    fn predecessor(self, _db: &'db dyn Db, to: BirSourceType<'db>) -> Option<BirBoundaryType<'db>> {
        (to == BirSourceType::f64).then_some(BirBoundaryType::JsAny)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: BirBoundaryType<'db>,
        _to: BirSourceType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let cast_expr = Api::get(db).cast_to_f64(db);

        frag!({{
            let value = ~{in_expr};@
            ({cast_expr})?
        }})
    }
}

/// Casts an API-specific JavaScript boundary value to an owned `bool`.
pub struct CastJsAnyToBoolRule;

impl<'db> DirectCastRule<'db, BirBoundaryType<'db>, BirSourceType<'db>> for CastJsAnyToBoolRule {
    fn predecessor(self, _db: &'db dyn Db, to: BirSourceType<'db>) -> Option<BirBoundaryType<'db>> {
        (to == BirSourceType::bool).then_some(BirBoundaryType::JsAny)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: BirBoundaryType<'db>,
        _to: BirSourceType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let cast_expr = Api::get(db).cast_to_bool(db);

        frag!({{
            let value = ~{in_expr};@
            ({cast_expr})?
        }})
    }
}

/// Casts an API-specific JavaScript boundary value to an owned `String`.
pub struct CastJsAnyToStringRule;

impl<'db> DirectCastRule<'db, BirBoundaryType<'db>, BirSourceType<'db>> for CastJsAnyToStringRule {
    fn predecessor(self, _db: &'db dyn Db, to: BirSourceType<'db>) -> Option<BirBoundaryType<'db>> {
        (to == BirSourceType::String).then_some(BirBoundaryType::JsAny)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: BirBoundaryType<'db>,
        _to: BirSourceType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let cast_expr = Api::get(db).cast_to_string(db);

        frag!({{
            let value = ~{in_expr};@
            ({cast_expr})?
        }})
    }
}

/// Borrows an owned boundary `String` as `&str`.
pub struct CastStringToStrRefRule;

impl<'db> IndirectCastRule<'db, BirSourceType<'db>, BirSourceType<'db>> for CastStringToStrRefRule {
    fn predecessor(self, _db: &'db dyn Db, to: BirSourceType<'db>) -> Option<BirSourceType<'db>> {
        (to == BirSourceType::Ref {
            is_mutable: false,
            inner: Box::new(BirSourceType::Str),
        })
        .then_some(BirSourceType::String)
    }

    fn compile<'fragment, Expr, Continue>(
        self,
        _db: &'db dyn Db,
        _from: BirSourceType<'db>,
        _to: BirSourceType<'db>,
        in_expr: Expr,
        continue_with: Continue,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
        Continue: CastContinuation<'db>,
    {
        let continue_with = continue_with.compile("value");

        frag!({{
            let mut value = ~{in_expr};@
            let value = value.as_str();@
            {continue_with}
        }})
    }
}

/// Borrows an owned boundary `String` as `&mut str` for the continuation.
pub struct CastStringToStrMutRule;

impl<'db> IndirectCastRule<'db, BirSourceType<'db>, BirSourceType<'db>> for CastStringToStrMutRule {
    fn predecessor(self, _db: &'db dyn Db, to: BirSourceType<'db>) -> Option<BirSourceType<'db>> {
        (to == BirSourceType::Ref {
            is_mutable: true,
            inner: Box::new(BirSourceType::Str),
        })
        .then_some(BirSourceType::String)
    }

    fn compile<'fragment, Expr, Continue>(
        self,
        _db: &'db dyn Db,
        _from: BirSourceType<'db>,
        _to: BirSourceType<'db>,
        in_expr: Expr,
        continue_with: Continue,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
        Continue: CastContinuation<'db>,
    {
        let continue_with = continue_with.compile("value");

        frag!({{
            let mut value = ~{in_expr};@
            let value = value.as_mut_str();@
            {continue_with}
        }})
    }
}

#[cfg(test)]
mod tests {
    use crate::codegen::{
        api::{Api, ApiKind, WasmTarget},
        bindings::cast_graph::test::Consume,
        db::DbImpl,
        fragment::FragmentPrint,
        test::assert_snap,
    };

    use super::*;

    fn snapshot_for_apis(compile: impl Fn(&DbImpl) -> String) -> String {
        let mut db = DbImpl::default();

        Api::set(&mut db, ApiKind::Node);
        let node = compile(&db);
        syn::parse_str::<syn::Expr>(&node).unwrap_or_else(|error| {
            panic!("generated Node cast must be a valid Rust expression: `{node}`: {error}")
        });

        Api::set(
            &mut db,
            ApiKind::Wasm {
                target: WasmTarget::Web,
            },
        );
        let wasm = compile(&db);
        syn::parse_str::<syn::Expr>(&wasm).unwrap_or_else(|error| {
            panic!("generated Wasm cast must be a valid Rust expression: `{wasm}`: {error}")
        });

        ("// Node\n", node, "\n\n// Wasm\n", wasm).print()
    }

    macro_rules! direct_cast_snapshot_test {
        ($name:ident, $rule:expr, $from:expr, $to:expr) => {
            #[test]
            fn $name() {
                assert_snap!({
                    let from = $from;
                    let to = $to;

                    snapshot_for_apis(|db| {
                        DirectCastRule::compile($rule, db, from.clone(), to.clone(), "input")
                            .print()
                    })
                });
            }
        };
    }

    direct_cast_snapshot_test!(
        casts_f64_to_signed_int,
        CastF64ToSignedIntRule,
        BirSourceType::f64,
        BirSourceType::i32
    );
    direct_cast_snapshot_test!(
        casts_f64_to_unsigned_int,
        CastF64ToUnsignedIntRule,
        BirSourceType::f64,
        BirSourceType::u32
    );
    direct_cast_snapshot_test!(
        casts_u32_to_char,
        CastU32ToCharRule,
        BirSourceType::u32,
        BirSourceType::char
    );
    direct_cast_snapshot_test!(
        casts_f64_to_f32,
        CastF64ToF32Rule,
        BirSourceType::f64,
        BirSourceType::f32
    );
    direct_cast_snapshot_test!(
        casts_char_to_u32_boundary,
        CastCharToU32BoundaryRule,
        BirSourceType::char,
        BirBoundaryType::u32
    );
    direct_cast_snapshot_test!(
        casts_f32_to_f64_boundary,
        CastF32ToF64BoundaryRule,
        BirSourceType::f32,
        BirBoundaryType::f64
    );
    direct_cast_snapshot_test!(
        passes_primitive_to_same_boundary,
        CastPrimitiveToSameBoundaryRule,
        BirSourceType::i32,
        BirBoundaryType::i32
    );
    direct_cast_snapshot_test!(
        casts_boundary_js_type_to_f64,
        CastJsAnyToF64Rule,
        BirBoundaryType::JsAny,
        BirSourceType::f64
    );
    direct_cast_snapshot_test!(
        casts_boundary_js_type_to_bool,
        CastJsAnyToBoolRule,
        BirBoundaryType::JsAny,
        BirSourceType::bool
    );
    direct_cast_snapshot_test!(
        casts_boundary_js_type_to_string,
        CastJsAnyToStringRule,
        BirBoundaryType::JsAny,
        BirSourceType::String
    );

    #[test]
    fn casts_string_to_str_ref() {
        assert_snap!({
            let from = BirSourceType::String;
            let to = BirSourceType::Ref {
                is_mutable: false,
                inner: Box::new(BirSourceType::Str),
            };

            snapshot_for_apis(|db| {
                IndirectCastRule::compile(
                    CastStringToStrRefRule,
                    db,
                    from.clone(),
                    to.clone(),
                    "input",
                    Consume,
                )
                .print()
            })
        });
    }

    #[test]
    fn casts_string_to_str_mut() {
        assert_snap!({
            let from = BirSourceType::String;
            let to = BirSourceType::Ref {
                is_mutable: true,
                inner: Box::new(BirSourceType::Str),
            };

            snapshot_for_apis(|db| {
                IndirectCastRule::compile(
                    CastStringToStrMutRule,
                    db,
                    from.clone(),
                    to.clone(),
                    "input",
                    Consume,
                )
                .print()
            })
        });
    }
}
