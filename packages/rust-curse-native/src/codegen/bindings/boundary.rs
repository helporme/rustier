//! Module for compiling expressions to convert values from boundary type to
//! source type and vice versa.

use crate::codegen::{
    api::Api,
    bindings::{
        printer::Printer,
        projection::{compile_mut_access_stmt, compile_ref_access_stmt},
        struct_::{compile_binding_struct_accessor_expr, compile_binding_struct_ctor_expr},
    },
    db::Db,
    ir::{
        fn_::{FnParam, ParamMode},
        struct_::struct_binding_name_from_rustdoc_id,
        type_::{Primitive, Type},
        IrId,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BoundaryMode {
    Move,
    Ref,
    Mut,
}

/// Expression that needs to be converted from the binding type to the source
/// type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BoundaryInExpr<'db> {
    /// Expression, for example `a` or `a.src`.
    pub in_expr: &'db str,
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
pub fn compile_boundary_in_conversion_expr<'db>(
    db: &'db dyn Db,
    exprs: &[BoundaryInExpr<'db>],
    inner_expr: String,
) -> String {
    if exprs.is_empty() {
        return inner_expr;
    }

    let mut out = Printer::default();

    out.curlies(|out| {
        out.line("let mut result = std::mem::MaybeUninit::uninit();");
        out.lines(&compile_boundary_in_conversion_stmt(
            db,
            exprs,
            format!("result.write({inner_expr});"),
        ));
        out.line("unsafe { result.assume_init() }")
    });

    out.into()
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
pub fn compile_boundary_in_conversion_stmt<'db>(
    db: &'db dyn Db,
    exprs: &[BoundaryInExpr<'db>],
    mut inner_stmt: String,
) -> String {
    let mut out = Printer::default();

    out.curlies(|out| {
        for param in exprs {
            if matches!(param.type_, Type::Struct(_)) {
                inner_stmt = compile_boundary_conversion_expr_via_projection(param, &inner_stmt);
            }
        }

        for param in exprs {
            if !matches!(param.type_, Type::Struct(_)) {
                let cast = compile_boundary_conversion_stmt_via_cast(db, param);
                inner_stmt = format!("{cast}\n{inner_stmt}");
            }
        }

        out.lines(&inner_stmt);
    });

    out.into()
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
    expr: &BoundaryInExpr<'db>,
) -> String {
    assert!(!matches!(expr.type_, Type::Struct(_)));

    let api = Api::get(db);
    let mut out = Printer::default();

    let type_ = expr.type_;
    let error = api.error(
        db,
        &format!(
            "format!(\"Expected Rust `{}` from a JavaScript {}: {{}}\", err)",
            type_.rs_name(db),
            type_.ts_name(db)
        ),
    );

    let expr_to_convert = &expr.in_expr;
    let var_name = &expr.out_var_name;

    let ref_ = match expr.mode {
        BoundaryMode::Move => "",
        BoundaryMode::Ref => "& ",
        BoundaryMode::Mut => "&mut ",
    };
    let cast_fn_name = type_.cast_fn_name(db);

    out.line_fmt(format_args!(
        "let mut {var_name} = {ref_}{cast_fn_name}({expr_to_convert}).map_err(|err| {error})?;",
    ));

    out.into()
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
    expr: &BoundaryInExpr<'db>,
    inner_expr: &str,
) -> String {
    assert!(matches!(expr.type_, Type::Struct(_)));

    let src_access_expr = compile_binding_struct_accessor_expr(expr.in_expr);

    match expr.mode {
        BoundaryMode::Move => {
            let mut inner_wrap = Printer::default();
            inner_wrap.curlies(|out| {
                out.line_fmt(format_args!(
                    "let {} = {}.clone();",
                    expr.out_var_name, expr.out_var_name
                ));
                out.lines(inner_expr);
            });
            let inner_expr_wrap: String = inner_wrap.into();

            compile_ref_access_stmt(&src_access_expr, expr.out_var_name, &inner_expr_wrap)
        }
        BoundaryMode::Ref => {
            compile_ref_access_stmt(&src_access_expr, expr.out_var_name, inner_expr)
        }
        BoundaryMode::Mut => {
            compile_mut_access_stmt(&src_access_expr, expr.out_var_name, inner_expr)
        }
    }
}

/// Compiles the name of the boundary input type.
///
/// The value matching type can be obtained from the expression compiled by
/// [`compile_boundary_in_conversion_expr`].
///
/// For example, `Type::Primitive(Primitive::i32)` is compiled to `"JsValue"`
/// for active wasm api (see [`crate::codegen::api::Api::js_type`]).
pub fn compile_boundary_in_type_name<'db>(db: &'db dyn Db, ty: Type<'db>) -> &'db str {
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
        | Type::String => api.js_type(db),
        Type::Struct(id) => compile_boundary_in_struct_name(db, id),
    }
}

#[salsa::tracked(returns(ref))]
fn compile_boundary_in_struct_name<'db>(db: &'db dyn Db, id: IrId<'db>) -> String {
    let name = struct_binding_name_from_rustdoc_id(db, id.rustdoc_id(db));
    format!("&{}", name)
}

/// Expression that needs to be converted from the source type to the binding
/// type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BoundaryOutExpr<'db> {
    /// Expression to convert, for example `a` or `a.src`.
    pub in_expr: &'db str,
    /// Type of the expression.
    pub type_: Type<'db>,
}

/// Compiles an expression that converts a given expression from the source type
/// to the binding type.
///
/// ```ignore (incomplete expression)
/// 123.3f64 as f32
/// ```
pub fn compile_boundary_out_conversion_expr<'db>(
    db: &dyn Db,
    expr: &BoundaryOutExpr<'db>,
) -> String {
    match expr.type_ {
        Type::Primitive(Primitive::Char) => format!("{} as u32", expr.in_expr),
        Type::Primitive(Primitive::f32) => format!("{} as f64", expr.in_expr),
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
        | Type::String => expr.in_expr.to_owned(),
        Type::Str => unreachable!(
            "str cannot be returned from the function, since functions cannot return references"
        ),
        Type::Struct(id) => {
            compile_binding_struct_ctor_expr(db, id, expr.in_expr.to_owned()).to_owned()
        }
    }
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
