use crate::{
    codegen::{
        api::Api,
        bindings::boundary::{AccessMode, BoundaryJsType, BoundaryType},
        cast::{DirectCastRule, IndirectCastRule},
        db::Db,
        fragment::{FragmentOnce, doubles},
        ir::type_::{Primitive, Type},
    },
    frag,
};

/// Casts an owned boundary `f64` to any supported signed integer type.
pub struct CastF64ToSignedIntRule;

impl<'db> DirectCastRule<'db, BoundaryType<'db>, BoundaryType<'db>> for CastF64ToSignedIntRule {
    fn predecessor(self, _db: &'db dyn Db, to: BoundaryType<'db>) -> Option<BoundaryType<'db>> {
        (to.mode == AccessMode::Owned
            && matches!(
                to.inner,
                Type::Primitive(Primitive::i8 | Primitive::i16 | Primitive::i32)
            ))
        .then_some(BoundaryType {
            inner: Type::Primitive(Primitive::f64),
            mode: AccessMode::Owned,
        })
    }

    fn compile(
        self,
        db: &'db dyn Db,
        _from: BoundaryType<'db>,
        to: BoundaryType<'db>,
        in_expr: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
        let type_name = to.inner.fqn(db);
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

impl<'db> DirectCastRule<'db, BoundaryType<'db>, BoundaryType<'db>> for CastF64ToUnsignedIntRule {
    fn predecessor(self, _db: &'db dyn Db, to: BoundaryType<'db>) -> Option<BoundaryType<'db>> {
        (to.mode == AccessMode::Owned
            && matches!(
                to.inner,
                Type::Primitive(Primitive::u8 | Primitive::u16 | Primitive::u32)
            ))
        .then_some(BoundaryType {
            inner: Type::Primitive(Primitive::f64),
            mode: AccessMode::Owned,
        })
    }

    fn compile(
        self,
        db: &'db dyn Db,
        _from: BoundaryType<'db>,
        to: BoundaryType<'db>,
        in_expr: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
        let type_name = to.inner.fqn(db);
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

impl<'db> DirectCastRule<'db, BoundaryType<'db>, BoundaryType<'db>> for CastU32ToCharRule {
    fn predecessor(self, _db: &'db dyn Db, to: BoundaryType<'db>) -> Option<BoundaryType<'db>> {
        (to == BoundaryType {
            inner: Type::Primitive(Primitive::Char),
            mode: AccessMode::Owned,
        })
        .then_some(BoundaryType {
            inner: Type::Primitive(Primitive::u32),
            mode: AccessMode::Owned,
        })
    }

    fn compile(
        self,
        db: &'db dyn Db,
        _from: BoundaryType<'db>,
        _to: BoundaryType<'db>,
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

impl<'db> DirectCastRule<'db, BoundaryType<'db>, BoundaryType<'db>> for CastF64ToF32Rule {
    fn predecessor(self, _db: &'db dyn Db, to: BoundaryType<'db>) -> Option<BoundaryType<'db>> {
        (to == BoundaryType {
            inner: Type::Primitive(Primitive::f32),
            mode: AccessMode::Owned,
        })
        .then_some(BoundaryType {
            inner: Type::Primitive(Primitive::f64),
            mode: AccessMode::Owned,
        })
    }

    fn compile(
        self,
        _db: &'db dyn Db,
        _from: BoundaryType<'db>,
        _to: BoundaryType<'db>,
        in_expr: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
        frag!({{ {in_expr}~as f32 }})
    }
}

/// Casts an API-specific JavaScript boundary value to an owned `f64`.
pub struct CastBoundaryJsTypeToF64Rule;

impl<'db> DirectCastRule<'db, BoundaryJsType, BoundaryType<'db>> for CastBoundaryJsTypeToF64Rule {
    fn predecessor(self, _db: &'db dyn Db, to: BoundaryType<'db>) -> Option<BoundaryJsType> {
        (to == BoundaryType {
            inner: Type::Primitive(Primitive::f64),
            mode: AccessMode::Owned,
        })
        .then_some(BoundaryJsType)
    }

    fn compile(
        self,
        db: &'db dyn Db,
        _from: BoundaryJsType,
        _to: BoundaryType<'db>,
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
pub struct CastBoundaryJsTypeToBoolRule;

impl<'db> DirectCastRule<'db, BoundaryJsType, BoundaryType<'db>> for CastBoundaryJsTypeToBoolRule {
    fn predecessor(self, _db: &'db dyn Db, to: BoundaryType<'db>) -> Option<BoundaryJsType> {
        (to == BoundaryType {
            inner: Type::Primitive(Primitive::Boolean),
            mode: AccessMode::Owned,
        })
        .then_some(BoundaryJsType)
    }

    fn compile(
        self,
        db: &'db dyn Db,
        _from: BoundaryJsType,
        _to: BoundaryType<'db>,
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
pub struct CastBoundaryJsTypeToStringRule;

impl<'db> DirectCastRule<'db, BoundaryJsType, BoundaryType<'db>>
    for CastBoundaryJsTypeToStringRule
{
    fn predecessor(self, _db: &'db dyn Db, to: BoundaryType<'db>) -> Option<BoundaryJsType> {
        (to == BoundaryType {
            inner: Type::String,
            mode: AccessMode::Owned,
        })
        .then_some(BoundaryJsType)
    }

    fn compile(
        self,
        db: &'db dyn Db,
        _from: BoundaryJsType,
        _to: BoundaryType<'db>,
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

impl<'db> DirectCastRule<'db, BoundaryType<'db>, BoundaryType<'db>> for CastStringToStrRefRule {
    fn predecessor(self, _db: &'db dyn Db, to: BoundaryType<'db>) -> Option<BoundaryType<'db>> {
        (to == BoundaryType {
            inner: Type::Str,
            mode: AccessMode::Ref,
        })
        .then_some(BoundaryType {
            inner: Type::String,
            mode: AccessMode::Owned,
        })
    }

    fn compile(
        self,
        _db: &'db dyn Db,
        _from: BoundaryType<'db>,
        _to: BoundaryType<'db>,
        in_expr: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
        frag!({ in_expr }.as_str())
    }
}

/// Borrows an owned boundary `String` as `&mut str` for the continuation.
pub struct CastStringToStrMutRule;

impl<'db> IndirectCastRule<'db, BoundaryType<'db>, BoundaryType<'db>> for CastStringToStrMutRule {
    fn predecessor(self, _db: &'db dyn Db, to: BoundaryType<'db>) -> Option<BoundaryType<'db>> {
        (to == BoundaryType {
            inner: Type::Str,
            mode: AccessMode::Mut,
        })
        .then_some(BoundaryType {
            inner: Type::String,
            mode: AccessMode::Owned,
        })
    }

    fn compile(
        self,
        _db: &'db dyn Db,
        _from: BoundaryType<'db>,
        _to: BoundaryType<'db>,
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
        BoundaryType {
            inner: Type::Primitive(Primitive::f64),
            mode: AccessMode::Owned,
        },
        BoundaryType {
            inner: Type::Primitive(Primitive::i32),
            mode: AccessMode::Owned,
        }
    );
    direct_cast_snapshot_test!(
        casts_f64_to_unsigned_int,
        CastF64ToUnsignedIntRule,
        BoundaryType {
            inner: Type::Primitive(Primitive::f64),
            mode: AccessMode::Owned,
        },
        BoundaryType {
            inner: Type::Primitive(Primitive::u32),
            mode: AccessMode::Owned,
        }
    );
    direct_cast_snapshot_test!(
        casts_u32_to_char,
        CastU32ToCharRule,
        BoundaryType {
            inner: Type::Primitive(Primitive::u32),
            mode: AccessMode::Owned,
        },
        BoundaryType {
            inner: Type::Primitive(Primitive::Char),
            mode: AccessMode::Owned,
        }
    );
    direct_cast_snapshot_test!(
        casts_f64_to_f32,
        CastF64ToF32Rule,
        BoundaryType {
            inner: Type::Primitive(Primitive::f64),
            mode: AccessMode::Owned,
        },
        BoundaryType {
            inner: Type::Primitive(Primitive::f32),
            mode: AccessMode::Owned,
        }
    );
    direct_cast_snapshot_test!(
        casts_boundary_js_type_to_f64,
        CastBoundaryJsTypeToF64Rule,
        BoundaryJsType,
        BoundaryType {
            inner: Type::Primitive(Primitive::f64),
            mode: AccessMode::Owned,
        }
    );
    direct_cast_snapshot_test!(
        casts_boundary_js_type_to_bool,
        CastBoundaryJsTypeToBoolRule,
        BoundaryJsType,
        BoundaryType {
            inner: Type::Primitive(Primitive::Boolean),
            mode: AccessMode::Owned,
        }
    );
    direct_cast_snapshot_test!(
        casts_boundary_js_type_to_string,
        CastBoundaryJsTypeToStringRule,
        BoundaryJsType,
        BoundaryType {
            inner: Type::String,
            mode: AccessMode::Owned,
        }
    );
    direct_cast_snapshot_test!(
        casts_string_to_str_ref,
        CastStringToStrRefRule,
        BoundaryType {
            inner: Type::String,
            mode: AccessMode::Owned,
        },
        BoundaryType {
            inner: Type::Str,
            mode: AccessMode::Ref,
        }
    );

    #[test]
    fn casts_string_to_str_mut() {
        assert_snap!({
            let from = BoundaryType {
                inner: Type::String,
                mode: AccessMode::Owned,
            };
            let to = BoundaryType {
                inner: Type::Str,
                mode: AccessMode::Mut,
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
