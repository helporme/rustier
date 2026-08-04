use crate::{
    compiler::{
        api::Api,
        bindings::bir::ExternType,
        cast::{CastContinuation, DirectCastRule, IndirectCastRule},
        db::Db,
        fragment::{FragmentOnce, doubles},
        ir::{Type, ValueType},
    },
    frag,
};

/// Casts an owned boundary `f64` to any supported signed integer type.
pub struct CastF64ToSignedIntRule;

impl<'db> DirectCastRule<'db, Type<'db>, Type<'db>> for CastF64ToSignedIntRule {
    fn predecessor(self, _db: &'db dyn Db, to: Type<'db>) -> Option<Type<'db>> {
        matches!(
            to,
            Type::Value(ValueType::i8) | Type::Value(ValueType::i16) | Type::Value(ValueType::i32)
        )
        .then_some(Type::Value(ValueType::f64))
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: Type<'db>,
        to: Type<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let type_name = match to {
            Type::Value(ValueType::i8) => "i8",
            Type::Value(ValueType::i16) => "i16",
            Type::Value(ValueType::i32) => "i32",
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

impl<'db> DirectCastRule<'db, Type<'db>, Type<'db>> for CastF64ToUnsignedIntRule {
    fn predecessor(self, _db: &'db dyn Db, to: Type<'db>) -> Option<Type<'db>> {
        matches!(
            to,
            Type::Value(ValueType::u8) | Type::Value(ValueType::u16) | Type::Value(ValueType::u32)
        )
        .then_some(Type::Value(ValueType::f64))
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: Type<'db>,
        to: Type<'db>,
        in_expr: Expr,
    ) -> impl FragmentOnce + 'fragment
    where
        'db: 'fragment,
        Expr: FragmentOnce + 'fragment,
    {
        let type_name = match to {
            Type::Value(ValueType::u8) => "u8",
            Type::Value(ValueType::u16) => "u16",
            Type::Value(ValueType::u32) => "u32",
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

impl<'db> DirectCastRule<'db, Type<'db>, Type<'db>> for CastU32ToCharRule {
    fn predecessor(self, _db: &'db dyn Db, to: Type<'db>) -> Option<Type<'db>> {
        (to == Type::Value(ValueType::char)).then_some(Type::Value(ValueType::u32))
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: Type<'db>,
        _to: Type<'db>,
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

impl<'db> DirectCastRule<'db, Type<'db>, Type<'db>> for CastF64ToF32Rule {
    fn predecessor(self, _db: &'db dyn Db, to: Type<'db>) -> Option<Type<'db>> {
        (to == Type::Value(ValueType::f32)).then_some(Type::Value(ValueType::f64))
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: Type<'db>,
        _to: Type<'db>,
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

impl<'db> DirectCastRule<'db, Type<'db>, ExternType<'db>> for CastCharToU32BoundaryRule {
    fn predecessor(self, _db: &'db dyn Db, to: ExternType<'db>) -> Option<Type<'db>> {
        (to == ExternType::u32).then_some(Type::Value(ValueType::char))
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: Type<'db>,
        _to: ExternType<'db>,
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

impl<'db> DirectCastRule<'db, Type<'db>, ExternType<'db>> for CastF32ToF64BoundaryRule {
    fn predecessor(self, _db: &'db dyn Db, to: ExternType<'db>) -> Option<Type<'db>> {
        (to == ExternType::f64).then_some(Type::Value(ValueType::f32))
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: Type<'db>,
        _to: ExternType<'db>,
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

impl<'db> DirectCastRule<'db, Type<'db>, ExternType<'db>> for CastPrimitiveToSameBoundaryRule {
    fn predecessor(self, _db: &'db dyn Db, to: ExternType<'db>) -> Option<Type<'db>> {
        match to {
            ExternType::String => Some(Type::Value(ValueType::String)),
            ExternType::bool => Some(Type::Value(ValueType::bool)),
            ExternType::i8 => Some(Type::Value(ValueType::i8)),
            ExternType::u8 => Some(Type::Value(ValueType::u8)),
            ExternType::i16 => Some(Type::Value(ValueType::i16)),
            ExternType::u16 => Some(Type::Value(ValueType::u16)),
            ExternType::i32 => Some(Type::Value(ValueType::i32)),
            ExternType::u32 => Some(Type::Value(ValueType::u32)),
            ExternType::f64 => Some(Type::Value(ValueType::f64)),
            ExternType::JsAny | ExternType::Item { .. } | ExternType::Ref { .. } => None,
        }
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: Type<'db>,
        _to: ExternType<'db>,
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

impl<'db> DirectCastRule<'db, ExternType<'db>, Type<'db>> for CastJsAnyToF64Rule {
    fn predecessor(self, _db: &'db dyn Db, to: Type<'db>) -> Option<ExternType<'db>> {
        (to == Type::Value(ValueType::f64)).then_some(ExternType::JsAny)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: ExternType<'db>,
        _to: Type<'db>,
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

impl<'db> DirectCastRule<'db, ExternType<'db>, Type<'db>> for CastJsAnyToBoolRule {
    fn predecessor(self, _db: &'db dyn Db, to: Type<'db>) -> Option<ExternType<'db>> {
        (to == Type::Value(ValueType::bool)).then_some(ExternType::JsAny)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: ExternType<'db>,
        _to: Type<'db>,
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

impl<'db> DirectCastRule<'db, ExternType<'db>, Type<'db>> for CastJsAnyToStringRule {
    fn predecessor(self, _db: &'db dyn Db, to: Type<'db>) -> Option<ExternType<'db>> {
        (to == Type::Value(ValueType::String)).then_some(ExternType::JsAny)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: ExternType<'db>,
        _to: Type<'db>,
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

impl<'db> IndirectCastRule<'db, Type<'db>, Type<'db>> for CastStringToStrRefRule {
    fn predecessor(self, _db: &'db dyn Db, to: Type<'db>) -> Option<Type<'db>> {
        (to == Type::Value(ValueType::Str).into_ref()).then_some(Type::Value(ValueType::String))
    }

    fn compile<'fragment, Expr, Continue>(
        self,
        _db: &'db dyn Db,
        _from: Type<'db>,
        _to: Type<'db>,
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

impl<'db> IndirectCastRule<'db, Type<'db>, Type<'db>> for CastStringToStrMutRule {
    fn predecessor(self, _db: &'db dyn Db, to: Type<'db>) -> Option<Type<'db>> {
        (to == Type::Value(ValueType::Str).into_mut()).then_some(Type::Value(ValueType::String))
    }

    fn compile<'fragment, Expr, Continue>(
        self,
        _db: &'db dyn Db,
        _from: Type<'db>,
        _to: Type<'db>,
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
