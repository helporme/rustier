//! Module for compiling expressions to convert values from boundary type to
//! source type and vice versa.

use crate::{
    codegen::{
        api::Api,
        bindings::{
            projection::{compile_mut_access_stmt, compile_ref_access_stmt},
            struct_::{compile_binding_struct_accessor_expr, compile_binding_struct_ctor_expr},
        },
        db::Db,
        fragment::{Fragment, FragmentOnce, FragmentPrint, fragment},
        ir::{
            IrId,
            fn_::{FnParam, ParamMode},
            struct_::struct_binding_name_from_rustdoc_id,
            type_::{Primitive, Type},
        },
    },
    frag,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BoundaryMode {
    Move,
    Ref,
    Mut,
}

/// Expression that needs to be converted from the binding type to the source
/// type.
#[derive(Copy, Clone)]
pub struct BoundaryInExpr<'db> {
    /// Expression, for example `a` or `a.src`.
    pub in_expr: &'db dyn Fragment,
    /// Name of the variable to which the converted value will be assigned.
    pub out_var_name: &'db str,
    /// Type of the expression.
    pub type_: Type<'db>,
    /// Whether the expression type is a mutable reference.
    pub mode: BoundaryMode,
}

impl<'db> BoundaryInExpr<'db> {
    /// Converts a [`FnParam`] to a [`BoundaryInExpr`] using the parameter's
    /// binding name as both `in_expr` and `out_var_name`.
    pub fn from_param(db: &'db dyn Db, param: FnParam<'db>) -> Self {
        Self {
            in_expr: param.binding_name(db),
            out_var_name: param.binding_name(db),
            type_: param.type_(db),
            mode: match param.mode(db) {
                ParamMode::Ref => BoundaryMode::Ref,
                ParamMode::Mut => BoundaryMode::Mut,
                ParamMode::Move => BoundaryMode::Move,
            },
        }
    }
}

/// Compiles an expression that converts each given expression to the source
/// type and exposes resulting variables to the `inner` expression.
///
/// A result returned from the `inner_expr` is returned from the compiled
/// expression.
///
/// The expression should be inserted into a function that returns the
/// [`crate::codegen::api::Api::result_type`] type.
///
/// Illustrative example of compiled expression:
/// ```ignore (incomplete expression)
/// {
///    let result = None;
///    {
///      let a = __cast_u32(a); // +map_err
///      b.src.with_mut(|b| {
///         result = Some(/* inner */)
///      }) // +map_err
///    }
///    result.unwrap()
/// }
/// ```
pub fn compile_boundary_in_conversion_expr<'db, E>(
    db: &'db dyn Db,
    exprs: E,
    inner_expr: impl FragmentOnce + 'db,
) -> impl FragmentOnce + 'db
where
    E: IntoIterator<Item = BoundaryInExpr<'db>> + 'db,
    E::IntoIter: Clone,
{
    fragment(move |p| {
        let exprs = exprs.into_iter();
        if exprs.clone().next().is_none() {
            inner_expr.apply(p);
            return;
        }

        frag!({
            {
                let mut result = std::mem::MaybeUninit::uninit();
                {
                    compile_boundary_in_conversion_stmt(
                        db,
                        exprs,
                        frag!(result.write({inner_expr});),
                    )
                }@
                unsafe~{{
                    result.assume_init()
                }}
            }
        })
        .apply(p);
    })
}

/// Compiles a statement that converts each given expression to the source
/// type and exposes resulting variables to the `inner` expression.
///
/// The statement should be inserted into a function that returns the
/// [`crate::codegen::api::Api::result_type`] type.
///
/// Illustrative example of compiled expression:
/// ```ignore (incomplete expression)
/// {
///    let a = __cast_u32(a); // +map_err
///    b.src.with_mut(|b| {
///        /* inner */
///    }) // +map_err
/// }
/// ```
pub fn compile_boundary_in_conversion_stmt<'db, E>(
    db: &'db dyn Db,
    exprs: E,
    inner_stmt: impl FragmentOnce + 'db,
) -> impl FragmentOnce + 'db
where
    E: IntoIterator<Item = BoundaryInExpr<'db>> + 'db,
    E::IntoIter: Clone,
{
    let exprs = exprs.into_iter();
    let will_use_projection_closures = exprs
        .clone()
        .any(|expr| matches!(expr.type_, Type::Struct(_)));

    let cast_exprs = exprs.clone();
    let cast_stmts = fragment(move |p| {
        for expr in cast_exprs {
            if !matches!(expr.type_, Type::Struct(_)) {
                frag!({compile_boundary_conversion_stmt_via_cast(db, expr)}@).apply(p);

                // In case projections are used, we wrap strings in `Some` on move,
                // so we can take them out safely without violating the FnMut contract of
                // projection closures.
                if will_use_projection_closures
                    && matches!((expr.type_, expr.mode), (Type::String, BoundaryMode::Move))
                {
                    frag!(
                        let mut ~{expr.out_var_name}~ = Some({expr.out_var_name});@
                    )
                    .apply(p);
                }
            }
        }
    });

    let inner_stmt_exprs = exprs.clone();
    let inner_stmt_prelude = fragment(move |p| {
        for expr in inner_stmt_exprs {
            // Structs are cloned on move. To achieve it, we are accessing the source
            // structs by ref projection and cloning them inside the clojure.
            //
            // It's important to clone the struct inside the clojure, rather than
            // before it, to avoid moving the struct out of the closure's scope, since
            // projection fn takes FnMut and not FnOnce.
            if matches!(
                (expr.type_, expr.mode),
                (Type::Struct(_), BoundaryMode::Move)
            ) {
                frag!(
                    let ~{expr.out_var_name}~ = ~{expr.out_var_name}.clone();@
                )
                .apply(p);
            }

            // In case projections will are used, we wrap strings in `Some` on
            // move, so we can take them out safely without violating the FnMut
            // contract.
            if will_use_projection_closures
                && matches!((expr.type_, expr.mode), (Type::String, BoundaryMode::Move))
            {
                frag!(
                    let ~{expr.out_var_name}~ = ~{expr.out_var_name}.take().unwrap();@
                )
                .apply(p);
            }
        }
    });

    let mut projection_expr: String = frag!(
        {inner_stmt_prelude}@
        {inner_stmt}
    )
    .print();

    for expr in exprs.clone() {
        if matches!(expr.type_, Type::Struct(_)) {
            projection_expr =
                compile_boundary_conversion_expr_via_projection(expr, projection_expr).print();
        }
    }

    frag!(
        {cast_stmts}
        {projection_expr}
    )
}

/// Compiles a conversion statement that converts the given expression
/// to the source type using appropriate [`crate::codegen::bindings::cast`]
/// function. Result is stored in a variable specified in `expr`.
///
/// Appliable only to the expression of a type that can be casted, for example
/// primitives.
///
/// The expression should be inserted into a function that returns the
/// [`crate::codegen::api::Api::result_type`] type.
///
/// Illustrative example of compiled expression:
/// ```ignore (incomplete expression)
/// let a = __cast_u32(a); // +map_err
/// ```
fn compile_boundary_conversion_stmt_via_cast<'db>(
    db: &'db dyn Db,
    expr: BoundaryInExpr<'db>,
) -> impl FragmentOnce + 'db {
    assert!(!matches!(expr.type_, Type::Struct(_)));

    let api = Api::get(db);

    let error = api.error(
        db,
        &format!(
            "format!(\"Expected Rust `{}` from a JavaScript {}: {{}}\", err)",
            expr.type_.rs_name(db),
            expr.type_.ts_name(db)
        ),
    );

    let ref_ = match expr.mode {
        BoundaryMode::Move => "",
        BoundaryMode::Ref => "& ",
        BoundaryMode::Mut => "&mut ",
    };
    let cast_fn_name = expr.type_.cast_fn_name(db);

    frag!(
        let ~{expr.out_var_name}~ = ~{ref_}{cast_fn_name}({expr.in_expr}).map_err(|err| {error})?;
    )
}

/// Compiles a conversion expression that converts the given expression
/// to the source type using appropriate
/// [`crate::codegen::bindings::projection`] function. Result is stored in a
/// variable specified in `expr`.
///
/// Can be applied only to a boundary type that implements `Projection`, for
/// example a struct.
///
/// Illustrative example of compiled expression:
/// ```ignore (incomplete expression)
/// a.src.with_mut(|src| {
///    /* inner */
/// }); // +map_err
/// ```
fn compile_boundary_conversion_expr_via_projection<'db>(
    expr: BoundaryInExpr<'db>,
    inner_expr: impl FragmentOnce + 'db,
) -> impl FragmentOnce + 'db {
    assert!(matches!(expr.type_, Type::Struct(_)));

    fragment(move |p| {
        let src_access_expr = compile_binding_struct_accessor_expr(expr.in_expr);

        match expr.mode {
            // cloning inside move is handled in a caller function, since we nede in advance know
            // all exprs that are going to be converted
            BoundaryMode::Ref | BoundaryMode::Move => {
                compile_ref_access_stmt(src_access_expr, expr.out_var_name, inner_expr).apply(p);
            }
            BoundaryMode::Mut => {
                compile_mut_access_stmt(src_access_expr, expr.out_var_name, inner_expr).apply(p);
            }
        }
    })
}

/// Compiles the name of the boundary input type.
///
/// The value matching type can be obtained from the expression compiled by
/// [`compile_boundary_in_conversion_expr`].
///
/// For example, `Type::Primitive(Primitive::i32)` is compiled to `"JsValue"`
/// for active wasm api (see [`crate::codegen::api::Api::js_type`]).
pub fn compile_boundary_in_type_name<'db>(
    db: &'db dyn Db,
    ty: Type<'db>,
) -> impl FragmentOnce + 'db {
    fragment(move |p| {
        let api = Api::get(db);

        match ty {
            Type::Primitive(
                Primitive::Boolean
                | Primitive::i8
                | Primitive::u8
                | Primitive::u16
                | Primitive::i16
                | Primitive::i32
                | Primitive::u32
                | Primitive::f32
                | Primitive::f64
                | Primitive::Char,
            )
            | Type::Str
            | Type::String => api.js_type(db).apply(p),
            Type::Struct(id) => compile_boundary_in_struct_name(db, id).apply(p),
        }
    })
}

fn compile_boundary_in_struct_name<'db>(db: &'db dyn Db, id: IrId<'db>) -> impl FragmentOnce + 'db {
    let name = struct_binding_name_from_rustdoc_id(db, id.rustdoc_id(db));
    frag!(&{ name })
}

/// Compiles an expression that converts a given expression from the source type
/// to the binding type.
///
/// ```ignore (incomplete expression)
/// 123.3f64 as f32
/// ```
pub fn compile_boundary_out_conversion_expr<'db>(
    db: &'db dyn Db,
    in_expr: impl FragmentOnce + 'db,
    in_expr_type: Type<'db>,
) -> impl FragmentOnce + 'db {
    fragment(move |p| {
        match in_expr_type {
            Type::Primitive(Primitive::Char) => frag!({ in_expr } as u32).apply(p),
            Type::Primitive(Primitive::f32) => frag!({ in_expr } as f64).apply(p),
            // can be returned as-is, since all APIs are supporting it natively
            Type::Primitive(
                Primitive::Boolean
                | Primitive::i8
                | Primitive::u8
                | Primitive::i16
                | Primitive::u16
                | Primitive::i32
                | Primitive::u32
                | Primitive::f64,
            )
            | Type::String => in_expr.apply(p),
            Type::Str => unreachable!(
                "str cannot be returned from the function, since functions cannot return references"
            ),
            Type::Struct(id) => compile_binding_struct_ctor_expr(db, id, in_expr).apply(p),
        }
    })
}

/// Compiles the name of the boundary output type.
///
/// The value matching type can be obtained from the expression compiled by
/// [`compile_boundary_out_conversion_expr`].
///
/// For example, `Type::Primitive(Primitive::i32)` is compiled to `"i32"`.
pub fn compile_boundary_out_type_name<'db>(db: &'db dyn Db, ty: Type<'db>) -> &'db str {
    match ty {
        Type::Primitive(Primitive::Boolean) => "bool",
        Type::Primitive(Primitive::i8) => "i8",
        Type::Primitive(Primitive::u8) => "u8",
        Type::Primitive(Primitive::i16) => "i16",
        Type::Primitive(Primitive::u16) => "u16",
        Type::Primitive(Primitive::i32) => "i32",
        Type::Primitive(Primitive::u32) | Type::Primitive(Primitive::Char) => "u32",
        Type::Primitive(Primitive::f64) | Type::Primitive(Primitive::f32) => "f64",
        Type::String => "String",
        Type::Str => unreachable!(
            "str cannot be returned from the function, since functions cannot return references"
        ),
        Type::Struct(id) => struct_binding_name_from_rustdoc_id(db, id.rustdoc_id(db)),
    }
}
