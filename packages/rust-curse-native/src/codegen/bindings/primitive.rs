use crate::{
    codegen::{
        api::Api,
        bindings::type_::{BoundaryType, PassMode, SourceType},
        cast::{DirectCastRule, IndirectCastRule},
        db::Db,
        fragment::{FragmentOnce, doubles},
        ir::type_::{Primitive, Type},
    },
    frag,
};

/// Casts an owned boundary `f64` to any supported signed integer type.
pub struct CastF64ToSignedIntRule;

impl<'db> DirectCastRule<'db, SourceType<'db>, SourceType<'db>> for CastF64ToSignedIntRule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<SourceType<'db>> {
        (to.mode == PassMode::Owned
            && matches!(
                to.value,
                Type::Primitive(Primitive::i8 | Primitive::i16 | Primitive::i32)
            ))
        .then_some(SourceType {
            value: Type::Primitive(Primitive::f64),
            mode: PassMode::Owned,
        })
    }

    fn compile(
        self,
        db: &'db dyn Db,
        _from: SourceType<'db>,
        to: SourceType<'db>,
        in_expr: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
        let type_name = to.value.fqn(db);
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
        (to.mode == PassMode::Owned
            && matches!(
                to.value,
                Type::Primitive(Primitive::u8 | Primitive::u16 | Primitive::u32)
            ))
        .then_some(SourceType {
            value: Type::Primitive(Primitive::f64),
            mode: PassMode::Owned,
        })
    }

    fn compile(
        self,
        db: &'db dyn Db,
        _from: SourceType<'db>,
        to: SourceType<'db>,
        in_expr: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
        let type_name = to.value.fqn(db);
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
        (to == SourceType {
            value: Type::Primitive(Primitive::Char),
            mode: PassMode::Owned,
        })
        .then_some(SourceType {
            value: Type::Primitive(Primitive::u32),
            mode: PassMode::Owned,
        })
    }

    fn compile(
        self,
        db: &'db dyn Db,
        _from: SourceType<'db>,
        _to: SourceType<'db>,
        in_expr: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
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
        (to == SourceType {
            value: Type::Primitive(Primitive::f32),
            mode: PassMode::Owned,
        })
        .then_some(SourceType {
            value: Type::Primitive(Primitive::f64),
            mode: PassMode::Owned,
        })
    }

    fn compile(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
        _to: SourceType<'db>,
        in_expr: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
        frag!({{ {in_expr}~as f32 }})
    }
}

/// Casts an API-specific JavaScript boundary value to an owned `f64`.
pub struct CastJsAnyToF64Rule;

impl<'db> DirectCastRule<'db, BoundaryType<'db>, SourceType<'db>> for CastJsAnyToF64Rule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<BoundaryType<'db>> {
        (to == SourceType {
            value: Type::Primitive(Primitive::f64),
            mode: PassMode::Owned,
        })
        .then_some(BoundaryType::JsAny)
    }

    fn compile(
        self,
        db: &'db dyn Db,
        _from: BoundaryType<'db>,
        _to: SourceType<'db>,
        in_expr: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
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
        (to == SourceType {
            value: Type::Primitive(Primitive::Boolean),
            mode: PassMode::Owned,
        })
        .then_some(BoundaryType::JsAny)
    }

    fn compile(
        self,
        db: &'db dyn Db,
        _from: BoundaryType<'db>,
        _to: SourceType<'db>,
        in_expr: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
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
        (to == SourceType {
            value: Type::String,
            mode: PassMode::Owned,
        })
        .then_some(BoundaryType::JsAny)
    }

    fn compile(
        self,
        db: &'db dyn Db,
        _from: BoundaryType<'db>,
        _to: SourceType<'db>,
        in_expr: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
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
        (to == SourceType {
            value: Type::Str,
            mode: PassMode::Ref,
        })
        .then_some(SourceType {
            value: Type::String,
            mode: PassMode::Owned,
        })
    }

    fn compile(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
        _to: SourceType<'db>,
        in_expr: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
        frag!({ in_expr }.as_str())
    }
}

/// Borrows an owned boundary `String` as `&mut str` for the continuation.
pub struct CastStringToStrMutRule;

impl<'db> IndirectCastRule<'db, SourceType<'db>, SourceType<'db>> for CastStringToStrMutRule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<SourceType<'db>> {
        (to == SourceType {
            value: Type::Str,
            mode: PassMode::Mut,
        })
        .then_some(SourceType {
            value: Type::String,
            mode: PassMode::Owned,
        })
    }

    fn compile(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
        _to: SourceType<'db>,
        in_expr: impl FragmentOnce + 'db,
        continue_with: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
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
        db::DbImpl,
        fragment::{FragmentPrint, fragment},
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
                        DirectCastRule::compile($rule, db, from, to, "input").print()
                    })
                });
            }
        };
    }

    direct_cast_snapshot_test!(
        casts_f64_to_signed_int,
        CastF64ToSignedIntRule,
        SourceType {
            value: Type::Primitive(Primitive::f64),
            mode: PassMode::Owned,
        },
        SourceType {
            value: Type::Primitive(Primitive::i32),
            mode: PassMode::Owned,
        }
    );
    direct_cast_snapshot_test!(
        casts_f64_to_unsigned_int,
        CastF64ToUnsignedIntRule,
        SourceType {
            value: Type::Primitive(Primitive::f64),
            mode: PassMode::Owned,
        },
        SourceType {
            value: Type::Primitive(Primitive::u32),
            mode: PassMode::Owned,
        }
    );
    direct_cast_snapshot_test!(
        casts_u32_to_char,
        CastU32ToCharRule,
        SourceType {
            value: Type::Primitive(Primitive::u32),
            mode: PassMode::Owned,
        },
        SourceType {
            value: Type::Primitive(Primitive::Char),
            mode: PassMode::Owned,
        }
    );
    direct_cast_snapshot_test!(
        casts_f64_to_f32,
        CastF64ToF32Rule,
        SourceType {
            value: Type::Primitive(Primitive::f64),
            mode: PassMode::Owned,
        },
        SourceType {
            value: Type::Primitive(Primitive::f32),
            mode: PassMode::Owned,
        }
    );
    direct_cast_snapshot_test!(
        casts_boundary_js_type_to_f64,
        CastJsAnyToF64Rule,
        BoundaryType::JsAny,
        SourceType {
            value: Type::Primitive(Primitive::f64),
            mode: PassMode::Owned,
        }
    );
    direct_cast_snapshot_test!(
        casts_boundary_js_type_to_bool,
        CastJsAnyToBoolRule,
        BoundaryType::JsAny,
        SourceType {
            value: Type::Primitive(Primitive::Boolean),
            mode: PassMode::Owned,
        }
    );
    direct_cast_snapshot_test!(
        casts_boundary_js_type_to_string,
        CastJsAnyToStringRule,
        BoundaryType::JsAny,
        SourceType {
            value: Type::String,
            mode: PassMode::Owned,
        }
    );
    direct_cast_snapshot_test!(
        casts_string_to_str_ref,
        CastStringToStrRefRule,
        SourceType {
            value: Type::String,
            mode: PassMode::Owned,
        },
        SourceType {
            value: Type::Str,
            mode: PassMode::Ref,
        }
    );

    #[test]
    fn casts_string_to_str_mut() {
        assert_snap!({
            let from = SourceType {
                value: Type::String,
                mode: PassMode::Owned,
            };
            let to = SourceType {
                value: Type::Str,
                mode: PassMode::Mut,
            };

            snapshot_for_apis(|db| {
                IndirectCastRule::compile(
                    CastStringToStrMutRule,
                    db,
                    from,
                    to,
                    "input",
                    fragment(|p| "consume(value);".apply(p)),
                )
                .print()
            })
        });
    }
}
