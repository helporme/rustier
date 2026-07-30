use crate::{
    codegen::{
        api::Api,
        bindings::type_::{BoundaryType, SourceType},
        cast::{CastContinuation, DirectCastRule, IndirectCastRule},
        db::Db,
        fragment::{FragmentOnce, doubles},
    },
    frag,
};

/// Casts an owned boundary `f64` to any supported signed integer type.
pub struct CastF64ToSignedIntRule;

impl<'db> DirectCastRule<'db, SourceType<'db>, SourceType<'db>> for CastF64ToSignedIntRule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<SourceType<'db>> {
        matches!(to, SourceType::i8 | SourceType::i16 | SourceType::i32).then_some(SourceType::f64)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: SourceType<'db>,
        to: SourceType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let type_name = match to {
            SourceType::i8 => "i8",
            SourceType::i16 => "i16",
            SourceType::i32 => "i32",
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

impl<'db> DirectCastRule<'db, SourceType<'db>, SourceType<'db>> for CastF64ToUnsignedIntRule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<SourceType<'db>> {
        matches!(to, SourceType::u8 | SourceType::u16 | SourceType::u32).then_some(SourceType::f64)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: SourceType<'db>,
        to: SourceType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let type_name = match to {
            SourceType::u8 => "u8",
            SourceType::u16 => "u16",
            SourceType::u32 => "u32",
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

impl<'db> DirectCastRule<'db, SourceType<'db>, SourceType<'db>> for CastU32ToCharRule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<SourceType<'db>> {
        (to == SourceType::char).then_some(SourceType::u32)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: SourceType<'db>,
        _to: SourceType<'db>,
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

impl<'db> DirectCastRule<'db, SourceType<'db>, SourceType<'db>> for CastF64ToF32Rule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<SourceType<'db>> {
        (to == SourceType::f32).then_some(SourceType::f64)
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
        _to: SourceType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        frag!({{ {in_expr}~as f32 }})
    }
}

/// Casts an owned source `char` to a boundary `u32`.
pub struct CastCharToU32BoundaryRule;

impl<'db> DirectCastRule<'db, SourceType<'db>, BoundaryType<'db>> for CastCharToU32BoundaryRule {
    fn predecessor(self, _db: &'db dyn Db, to: BoundaryType<'db>) -> Option<SourceType<'db>> {
        (to == BoundaryType::u32).then_some(SourceType::char)
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
        _to: BoundaryType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        frag!({{ {in_expr}~as u32 }})
    }
}

/// Casts an owned source `f32` to a boundary `f64`.
pub struct CastF32ToF64BoundaryRule;

impl<'db> DirectCastRule<'db, SourceType<'db>, BoundaryType<'db>> for CastF32ToF64BoundaryRule {
    fn predecessor(self, _db: &'db dyn Db, to: BoundaryType<'db>) -> Option<SourceType<'db>> {
        (to == BoundaryType::f64).then_some(SourceType::f32)
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
        _to: BoundaryType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        frag!({in_expr}~as f64)
    }
}

/// Passes an owned primitive source value to its identical boundary type.
pub struct CastPrimitiveToSameBoundaryRule;

impl<'db> DirectCastRule<'db, SourceType<'db>, BoundaryType<'db>>
    for CastPrimitiveToSameBoundaryRule
{
    fn predecessor(self, _db: &'db dyn Db, to: BoundaryType<'db>) -> Option<SourceType<'db>> {
        match to {
            BoundaryType::String => Some(SourceType::String),
            BoundaryType::bool => Some(SourceType::bool),
            BoundaryType::i8 => Some(SourceType::i8),
            BoundaryType::u8 => Some(SourceType::u8),
            BoundaryType::i16 => Some(SourceType::i16),
            BoundaryType::u16 => Some(SourceType::u16),
            BoundaryType::i32 => Some(SourceType::i32),
            BoundaryType::u32 => Some(SourceType::u32),
            BoundaryType::f64 => Some(SourceType::f64),
            BoundaryType::f32 | BoundaryType::JsAny | BoundaryType::Item { .. } => None,
        }
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
        _to: BoundaryType<'db>,
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

impl<'db> DirectCastRule<'db, BoundaryType<'db>, SourceType<'db>> for CastJsAnyToF64Rule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<BoundaryType<'db>> {
        (to == SourceType::f64).then_some(BoundaryType::JsAny)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: BoundaryType<'db>,
        _to: SourceType<'db>,
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

impl<'db> DirectCastRule<'db, BoundaryType<'db>, SourceType<'db>> for CastJsAnyToBoolRule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<BoundaryType<'db>> {
        (to == SourceType::bool).then_some(BoundaryType::JsAny)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: BoundaryType<'db>,
        _to: SourceType<'db>,
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

impl<'db> DirectCastRule<'db, BoundaryType<'db>, SourceType<'db>> for CastJsAnyToStringRule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<BoundaryType<'db>> {
        (to == SourceType::String).then_some(BoundaryType::JsAny)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: BoundaryType<'db>,
        _to: SourceType<'db>,
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

impl<'db> DirectCastRule<'db, SourceType<'db>, SourceType<'db>> for CastStringToStrRefRule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<SourceType<'db>> {
        (to == SourceType::Ref {
            is_mutable: false,
            inner: Box::new(SourceType::Str),
        })
        .then_some(SourceType::String)
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
        _to: SourceType<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        frag!({ in_expr }.as_str())
    }
}

/// Borrows an owned boundary `String` as `&mut str` for the continuation.
pub struct CastStringToStrMutRule;

impl<'db> IndirectCastRule<'db, SourceType<'db>, SourceType<'db>> for CastStringToStrMutRule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<SourceType<'db>> {
        (to == SourceType::Ref {
            is_mutable: true,
            inner: Box::new(SourceType::Str),
        })
        .then_some(SourceType::String)
    }

    fn compile<'fragment, Expr, Continue>(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
        _to: SourceType<'db>,
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
        SourceType::f64,
        SourceType::i32
    );
    direct_cast_snapshot_test!(
        casts_f64_to_unsigned_int,
        CastF64ToUnsignedIntRule,
        SourceType::f64,
        SourceType::u32
    );
    direct_cast_snapshot_test!(
        casts_u32_to_char,
        CastU32ToCharRule,
        SourceType::u32,
        SourceType::char
    );
    direct_cast_snapshot_test!(
        casts_f64_to_f32,
        CastF64ToF32Rule,
        SourceType::f64,
        SourceType::f32
    );
    direct_cast_snapshot_test!(
        casts_char_to_u32_boundary,
        CastCharToU32BoundaryRule,
        SourceType::char,
        BoundaryType::u32
    );
    direct_cast_snapshot_test!(
        casts_f32_to_f64_boundary,
        CastF32ToF64BoundaryRule,
        SourceType::f32,
        BoundaryType::f64
    );
    direct_cast_snapshot_test!(
        passes_primitive_to_same_boundary,
        CastPrimitiveToSameBoundaryRule,
        SourceType::i32,
        BoundaryType::i32
    );
    direct_cast_snapshot_test!(
        casts_boundary_js_type_to_f64,
        CastJsAnyToF64Rule,
        BoundaryType::JsAny,
        SourceType::f64
    );
    direct_cast_snapshot_test!(
        casts_boundary_js_type_to_bool,
        CastJsAnyToBoolRule,
        BoundaryType::JsAny,
        SourceType::bool
    );
    direct_cast_snapshot_test!(
        casts_boundary_js_type_to_string,
        CastJsAnyToStringRule,
        BoundaryType::JsAny,
        SourceType::String
    );
    direct_cast_snapshot_test!(
        casts_string_to_str_ref,
        CastStringToStrRefRule,
        SourceType::String,
        SourceType::Ref {
            is_mutable: false,
            inner: Box::new(SourceType::Str),
        }
    );

    #[test]
    fn casts_string_to_str_mut() {
        assert_snap!({
            let from = SourceType::String;
            let to = SourceType::Ref {
                is_mutable: true,
                inner: Box::new(SourceType::Str),
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
