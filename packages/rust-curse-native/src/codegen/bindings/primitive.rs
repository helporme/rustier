use crate::{
    codegen::{
        api::Api,
        bindings::bir::{ExternType, SourceType},
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
        frag!(({in_expr})~as f32)
    }
}

/// Casts an owned source `char` to a boundary `u32`.
pub struct CastCharToU32BoundaryRule;

impl<'db> DirectCastRule<'db, SourceType<'db>, ExternType<'db>> for CastCharToU32BoundaryRule {
    fn predecessor(self, _db: &'db dyn Db, to: ExternType<'db>) -> Option<SourceType<'db>> {
        (to == ExternType::u32).then_some(SourceType::char)
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
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

impl<'db> DirectCastRule<'db, SourceType<'db>, ExternType<'db>> for CastF32ToF64BoundaryRule {
    fn predecessor(self, _db: &'db dyn Db, to: ExternType<'db>) -> Option<SourceType<'db>> {
        (to == ExternType::f64).then_some(SourceType::f32)
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
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

impl<'db> DirectCastRule<'db, SourceType<'db>, ExternType<'db>>
    for CastPrimitiveToSameBoundaryRule
{
    fn predecessor(self, _db: &'db dyn Db, to: ExternType<'db>) -> Option<SourceType<'db>> {
        match to {
            ExternType::String => Some(SourceType::String),
            ExternType::bool => Some(SourceType::bool),
            ExternType::i8 => Some(SourceType::i8),
            ExternType::u8 => Some(SourceType::u8),
            ExternType::i16 => Some(SourceType::i16),
            ExternType::u16 => Some(SourceType::u16),
            ExternType::i32 => Some(SourceType::i32),
            ExternType::u32 => Some(SourceType::u32),
            ExternType::f64 => Some(SourceType::f64),
            ExternType::JsAny | ExternType::Item { .. } | ExternType::Ref { .. } => None,
        }
    }

    fn compile<'fragment, Expr>(
        self,
        _db: &'db dyn Db,
        _from: SourceType<'db>,
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

impl<'db> DirectCastRule<'db, ExternType<'db>, SourceType<'db>> for CastJsAnyToF64Rule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<ExternType<'db>> {
        (to == SourceType::f64).then_some(ExternType::JsAny)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: ExternType<'db>,
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

impl<'db> DirectCastRule<'db, ExternType<'db>, SourceType<'db>> for CastJsAnyToBoolRule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<ExternType<'db>> {
        (to == SourceType::bool).then_some(ExternType::JsAny)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: ExternType<'db>,
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

impl<'db> DirectCastRule<'db, ExternType<'db>, SourceType<'db>> for CastJsAnyToStringRule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<ExternType<'db>> {
        (to == SourceType::String).then_some(ExternType::JsAny)
    }

    fn compile<'fragment, Expr>(
        self,
        db: &'db dyn Db,
        _from: ExternType<'db>,
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

impl<'db> IndirectCastRule<'db, SourceType<'db>, SourceType<'db>> for CastStringToStrRefRule {
    fn predecessor(self, _db: &'db dyn Db, to: SourceType<'db>) -> Option<SourceType<'db>> {
        (to == SourceType::Ref {
            is_mutable: false,
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
            let value = value.as_str();@
            {continue_with}
        }})
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
