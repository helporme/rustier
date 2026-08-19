//! Code to cast values from shim to source types back and forth

mod borrowing;
mod classes;
mod numbers;
mod strings;
mod tethering;

use crate::{
    compiler::{
        codegen::{
            fragment::{FragmentOnce, Printer, fragment},
            shim::tethering::TetheringStructType,
        },
        db::Db,
        ir::{
            rust::{Type, ValueType},
            shim::ShimType,
        },
    },
    frag,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastType<'db> {
    Tethered(TetheringStructType<'db>),
    Shim(ShimType<'db>),
    Rust(Type<'db>),
}

impl<'db> From<TetheringStructType<'db>> for CastType<'db> {
    fn from(value: TetheringStructType<'db>) -> Self {
        Self::Tethered(value)
    }
}

impl<'db> From<ShimType<'db>> for CastType<'db> {
    fn from(value: ShimType<'db>) -> Self {
        Self::Shim(value)
    }
}

impl<'db> From<Type<'db>> for CastType<'db> {
    fn from(value: Type<'db>) -> Self {
        Self::Rust(value)
    }
}

pub enum CastOutput<OutVar, Continue> {
    Expression,
    Continue {
        out_var_name: OutVar,
        continue_with: Continue,
    },
}

pub struct CastArgs<'db, InExpr, OutVar, Continue> {
    pub src: CastType<'db>,
    pub dst: CastType<'db>,
    pub in_expr: InExpr,
    pub output: CastOutput<OutVar, Continue>,
}

pub struct Cast<From, To, InExpr, OutVar> {
    pub from: From,
    pub to: To,
    pub in_expr: InExpr,
    pub out_var_name: OutVar,
}

pub trait CastRenderer<'db>: Copy + 'db {
    fn render<InExpr, OutVar, Continue>(
        self,
        db: &'db dyn Db,
        args: CastArgs<'db, InExpr, OutVar, Continue>,
    ) -> impl FragmentOnce + 'db
    where
        InExpr: FragmentOnce + 'db,
        OutVar: FragmentOnce + 'db,
        Continue: FragmentOnce + 'db;

    fn render_expr<From, To, InExpr>(
        self,
        db: &'db dyn Db,
        from: From,
        to: To,
        in_expr: InExpr,
    ) -> impl FragmentOnce + 'db
    where
        From: Into<CastType<'db>> + 'db,
        To: Into<CastType<'db>> + 'db,
        InExpr: FragmentOnce + 'db,
    {
        self.render(
            db,
            CastArgs {
                src: from.into(),
                dst: to.into(),
                in_expr,
                output: CastOutput::<(), ()>::Expression,
            },
        )
    }

    fn render_stmt<From, To, InExpr, OutVar, Continue>(
        self,
        db: &'db dyn Db,
        cast: Cast<From, To, InExpr, OutVar>,
        continue_with: Continue,
    ) -> impl FragmentOnce + 'db
    where
        From: Into<CastType<'db>> + 'db,
        To: Into<CastType<'db>> + 'db,
        InExpr: FragmentOnce + 'db,
        OutVar: FragmentOnce + 'db,
        Continue: FragmentOnce + 'db,
    {
        self.render(
            db,
            CastArgs {
                src: cast.from.into(),
                dst: cast.to.into(),
                in_expr: cast.in_expr,
                output: CastOutput::Continue {
                    out_var_name: cast.out_var_name,
                    continue_with,
                },
            },
        )
    }

    fn render_many<Casts, From, To, InExpr, OutVar, Continue>(
        self,
        db: &'db dyn Db,
        casts: Casts,
        continue_with: Continue,
    ) -> impl FragmentOnce + 'db
    where
        Casts: IntoIterator<Item = Cast<From, To, InExpr, OutVar>> + 'db,
        Casts::IntoIter: 'db,
        From: Into<CastType<'db>> + 'db,
        To: Into<CastType<'db>> + 'db,
        InExpr: FragmentOnce + 'db,
        OutVar: FragmentOnce + 'db,
        Continue: FragmentOnce + 'db,
    {
        RenderMany {
            renderer: self,
            db,
            casts: casts.into_iter(),
            continue_with,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Renderer;

impl<'db> CastRenderer<'db> for Renderer {
    fn render<InExpr, OutVar, Continue>(
        self,
        db: &'db dyn Db,
        args: CastArgs<'db, InExpr, OutVar, Continue>,
    ) -> impl FragmentOnce + 'db
    where
        InExpr: FragmentOnce + 'db,
        OutVar: FragmentOnce + 'db,
        Continue: FragmentOnce + 'db,
    {
        let CastArgs {
            src,
            dst,
            in_expr,
            output,
        } = args;

        fragment(move |printer| {
            use CastType::*;

            if src == dst {
                adapt_to_cast_output(in_expr, output).apply(printer);
                return;
            }

            match (src, dst) {
                // tethering
                (Rust(source), Tethered(target)) => {
                    tethering::render_type_to_tethered(db, source, target, in_expr, output)
                        .apply(printer);
                }
                (Tethered(source), Rust(target)) => {
                    tethering::render_tethered_to_type(db, source, target, in_expr, output)
                        .apply(printer);
                }

                // classes
                (Shim(class @ ShimType::Class { .. }), Tethered(_)) => {
                    classes::render_class_to_tethered(class, in_expr, output).apply(printer);
                }
                (Tethered(_), Shim(class @ ShimType::Class { .. })) => {
                    classes::render_tethered_to_class(db, class, in_expr, output).apply(printer);
                }
                (Shim(class @ ShimType::Class { .. }), Rust(target)) => {
                    classes::render_class_to_type(db, class, target, in_expr, output)
                        .apply(printer);
                }
                (Rust(source), Shim(class @ ShimType::Class { .. })) => {
                    classes::render_type_to_class(db, source, class, in_expr, output)
                        .apply(printer);
                }

                // numbers
                (Shim(ShimType::f64), Rust(target)) => {
                    numbers::render_f64_to_type(db, target, in_expr, output).apply(printer);
                }
                (Rust(source), Shim(ShimType::f64)) => {
                    numbers::render_type_to_f64(db, source, in_expr, output).apply(printer);
                }

                // booleans
                (
                    Shim(ShimType::bool),
                    Rust(
                        target @ Type::Borrowed {
                            inner: ValueType::bool,
                            ..
                        },
                    ),
                ) => {
                    borrowing::render_owned_to_borrowed(target, in_expr, output).apply(printer);
                }

                // strings
                (Shim(ShimType::String), Rust(target @ Type::Borrowed { .. })) => {
                    strings::render_string_to_type(target, in_expr, output).apply(printer);
                }

                // identity casts
                (Shim(ShimType::bool), Rust(Type::Value(ValueType::bool)))
                | (Rust(Type::Value(ValueType::bool)), Shim(ShimType::bool))
                | (Shim(ShimType::String), Rust(Type::Value(ValueType::String)))
                | (Rust(Type::Value(ValueType::String)), Shim(ShimType::String)) => {
                    adapt_to_cast_output(in_expr, output).apply(printer);
                }

                _ => unreachable!("unsupported shim cast from `{src:?}` to `{dst:?}`"),
            }
        })
    }
}

struct RenderMany<'db, Renderer, Casts, Continue> {
    renderer: Renderer,
    db: &'db dyn Db,
    casts: Casts,
    continue_with: Continue,
}

impl<'db, Renderer, Casts, From, To, InExpr, OutVar, Continue> FragmentOnce
    for RenderMany<'db, Renderer, Casts, Continue>
where
    Renderer: CastRenderer<'db> + 'db,
    Casts: Iterator<Item = Cast<From, To, InExpr, OutVar>> + 'db,
    From: Into<CastType<'db>> + 'db,
    To: Into<CastType<'db>> + 'db,
    InExpr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    fn apply(mut self, printer: &mut Printer) {
        let Some(cast) = self.casts.next() else {
            self.continue_with.apply(printer);
            return;
        };

        let remaining = Self {
            renderer: self.renderer,
            db: self.db,
            casts: self.casts,
            continue_with: self.continue_with,
        };

        self.renderer
            .render_stmt(self.db, cast, remaining)
            .apply(printer);
    }
}

/// Adapts an expression-producing cast to the requested output mode.
pub fn adapt_to_cast_output<'db, Expr, OutVar, Continue>(
    expr: Expr,
    output: CastOutput<OutVar, Continue>,
) -> impl FragmentOnce + 'db
where
    Expr: FragmentOnce + 'db,
    OutVar: FragmentOnce + 'db,
    Continue: FragmentOnce + 'db,
{
    fragment(move |printer| match output {
        CastOutput::Expression => expr.apply(printer),
        CastOutput::Continue {
            out_var_name,
            continue_with,
        } => frag!({{
            let ~{out_var_name}~ = ~{expr};
            {continue_with}
        }})
        .apply(printer),
    })
}

#[cfg(test)]
pub mod test {
    use std::fmt::{Debug, Write as _};

    use crate::{
        compiler::{
            codegen::fragment::{FragmentOnce, Printer},
            db::Db,
        },
        frag,
    };

    use super::{CastArgs, CastRenderer, adapt_to_cast_output};

    #[derive(Clone, Copy)]
    pub struct TestCastRenderer;

    struct DebugString<'db, T> {
        db: &'db dyn Db,
        value: T,
    }

    impl<T> FragmentOnce for DebugString<'_, T>
    where
        T: Debug,
    {
        fn apply(self, printer: &mut Printer) {
            let debug = salsa::attach(self.db, || format!("{:?}", self.value));
            write!(printer, "{debug:?}").expect("writing to source printer cannot fail");
        }
    }

    impl<'db> CastRenderer<'db> for TestCastRenderer {
        fn render<InExpr, OutVar, Continue>(
            self,
            db: &'db dyn Db,
            args: CastArgs<'db, InExpr, OutVar, Continue>,
        ) -> impl FragmentOnce + 'db
        where
            InExpr: FragmentOnce + 'db,
            OutVar: FragmentOnce + 'db,
            Continue: FragmentOnce + 'db,
        {
            let from = DebugString {
                db,
                value: args.src,
            };
            let to = DebugString {
                db,
                value: args.dst,
            };

            let expr = frag!(cast.from({ from }).to({ to }).an({ args.in_expr }));

            adapt_to_cast_output(expr, args.output)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::{
        codegen::shim::tethering::TetheringStructType,
        db::Db,
        ir::{
            rust::{Crate, Member, Type, ValueType},
            shim::ShimType,
        },
        test::TEST_CRATE_NAME,
    };

    macro_rules! cast_snapshot {
        (
            name: $name:ident,
            source: { $($source:tt)* },
            from: |$from_db:ident| $from:expr,
            to: |$to_db:ident| $to:expr $(,)?
        ) => {
            #[test]
            fn $name() {
                use $crate::frag;

                $crate::assert_rust_snap! {
                    source: { $($source)* },
                    expression: |__db, __printer| {
                        let __db: &dyn $crate::compiler::db::Db = &*__db;
                        let __from = {
                            let $from_db: &dyn $crate::compiler::db::Db = __db;
                            $from
                        };
                        let __to = {
                            let $to_db: &dyn $crate::compiler::db::Db = __db;
                            $to
                        };

                        $crate::compiler::codegen::fragment::FragmentOnce::apply(
                            $crate::frag!(
                                fn cast(input: ())~{{
                                    {$crate::compiler::codegen::shim::cast::CastRenderer::render_stmt(
                                        $crate::compiler::codegen::shim::cast::Renderer,
                                        __db,
                                        $crate::compiler::codegen::shim::cast::Cast {
                                            from: __from,
                                            to: __to,
                                            in_expr: "input",
                                            out_var_name: "output",
                                        },
                                        $crate::frag!(consume(output);),
                                    )}
                                }}
                            ),
                            __printer,
                        );
                    },
                }
            }
        };
        (
            name: $name:ident,
            from: |$from_db:ident| $from:expr,
            to: |$to_db:ident| $to:expr $(,)?
        ) => {
            cast_snapshot! {
                name: $name,
                source: {},
                from: |$from_db| $from,
                to: |$to_db| $to,
            }
        };
    }

    macro_rules! number_cast_snapshots {
        (
            value: $value:expr,
            in: $in_name:ident,
            ref_in: $ref_name:ident,
            mut_in: $mut_name:ident,
            out: $out_name:ident $(,)?
        ) => {
            cast_snapshot! {
                name: $in_name,
                from: |_db| ShimType::f64,
                to: |_db| Type::Value($value),
            }

            cast_snapshot! {
                name: $ref_name,
                from: |_db| ShimType::f64,
                to: |_db| $value.borrowed(),
            }

            cast_snapshot! {
                name: $mut_name,
                from: |_db| ShimType::f64,
                to: |_db| $value.borrowed_mut(),
            }

            cast_snapshot! {
                name: $out_name,
                from: |_db| Type::Value($value),
                to: |_db| ShimType::f64,
            }
        };
    }

    number_cast_snapshots! {
        value: ValueType::char,
        in: char_in,
        ref_in: char_ref_in,
        mut_in: char_mut_in,
        out: char_out,
    }

    number_cast_snapshots! {
        value: ValueType::i8,
        in: i8_in,
        ref_in: i8_ref_in,
        mut_in: i8_mut_in,
        out: i8_out,
    }

    number_cast_snapshots! {
        value: ValueType::u8,
        in: u8_in,
        ref_in: u8_ref_in,
        mut_in: u8_mut_in,
        out: u8_out,
    }

    number_cast_snapshots! {
        value: ValueType::i16,
        in: i16_in,
        ref_in: i16_ref_in,
        mut_in: i16_mut_in,
        out: i16_out,
    }

    number_cast_snapshots! {
        value: ValueType::u16,
        in: u16_in,
        ref_in: u16_ref_in,
        mut_in: u16_mut_in,
        out: u16_out,
    }

    number_cast_snapshots! {
        value: ValueType::i32,
        in: i32_in,
        ref_in: i32_ref_in,
        mut_in: i32_mut_in,
        out: i32_out,
    }

    number_cast_snapshots! {
        value: ValueType::u32,
        in: u32_in,
        ref_in: u32_ref_in,
        mut_in: u32_mut_in,
        out: u32_out,
    }

    number_cast_snapshots! {
        value: ValueType::f32,
        in: f32_in,
        ref_in: f32_ref_in,
        mut_in: f32_mut_in,
        out: f32_out,
    }

    number_cast_snapshots! {
        value: ValueType::f64,
        in: f64_in,
        ref_in: f64_ref_in,
        mut_in: f64_mut_in,
        out: f64_out,
    }

    cast_snapshot! {
        name: bool_in,
        from: |_db| ShimType::bool,
        to: |_db| Type::Value(ValueType::bool),
    }

    cast_snapshot! {
        name: bool_ref_in,
        from: |_db| ShimType::bool,
        to: |_db| ValueType::bool.borrowed(),
    }

    cast_snapshot! {
        name: bool_mut_in,
        from: |_db| ShimType::bool,
        to: |_db| ValueType::bool.borrowed_mut(),
    }

    cast_snapshot! {
        name: bool_out,
        from: |_db| Type::Value(ValueType::bool),
        to: |_db| ShimType::bool,
    }

    cast_snapshot! {
        name: string_in,
        from: |_db| ShimType::String,
        to: |_db| Type::Value(ValueType::String),
    }

    cast_snapshot! {
        name: string_ref_in,
        from: |_db| ShimType::String,
        to: |_db| ValueType::String.borrowed(),
    }

    cast_snapshot! {
        name: string_mut_in,
        from: |_db| ShimType::String,
        to: |_db| ValueType::String.borrowed_mut(),
    }

    cast_snapshot! {
        name: str_ref_in,
        from: |_db| ShimType::String,
        to: |_db| ValueType::Str.borrowed(),
    }

    cast_snapshot! {
        name: str_mut_in,
        from: |_db| ShimType::String,
        to: |_db| ValueType::Str.borrowed_mut(),
    }

    cast_snapshot! {
        name: string_out,
        from: |_db| Type::Value(ValueType::String),
        to: |_db| ShimType::String,
    }

    cast_snapshot! {
        name: struct_in,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| test_shim_class_type(db, "ExampleStruct", true),
        to: |db| Type::Value(test_value_type(db, "ExampleStruct")),
    }

    cast_snapshot! {
        name: struct_ref_in,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| test_shim_class_type(db, "ExampleStruct", true),
        to: |db| test_value_type(db, "ExampleStruct").borrowed(),
    }

    cast_snapshot! {
        name: struct_mut_in,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| test_shim_class_type(db, "ExampleStruct", true),
        to: |db| test_value_type(db, "ExampleStruct").borrowed_mut(),
    }

    cast_snapshot! {
        name: struct_out,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| Type::Value(test_value_type(db, "ExampleStruct")),
        to: |db| test_shim_class_type(db, "ExampleStruct", false),
    }

    cast_snapshot! {
        name: enum_in,
        source: {
            #[derive(Clone)]
            pub enum ExampleEnum {
                Variant,
            }
        },
        from: |db| test_shim_class_type(db, "ExampleEnum", true),
        to: |db| Type::Value(test_value_type(db, "ExampleEnum")),
    }

    cast_snapshot! {
        name: enum_ref_in,
        source: {
            #[derive(Clone)]
            pub enum ExampleEnum {
                Variant,
            }
        },
        from: |db| test_shim_class_type(db, "ExampleEnum", true),
        to: |db| test_value_type(db, "ExampleEnum").borrowed(),
    }

    cast_snapshot! {
        name: enum_mut_in,
        source: {
            #[derive(Clone)]
            pub enum ExampleEnum {
                Variant,
            }
        },
        from: |db| test_shim_class_type(db, "ExampleEnum", true),
        to: |db| test_value_type(db, "ExampleEnum").borrowed_mut(),
    }

    cast_snapshot! {
        name: enum_out,
        source: {
            #[derive(Clone)]
            pub enum ExampleEnum {
                Variant,
            }
        },
        from: |db| Type::Value(test_value_type(db, "ExampleEnum")),
        to: |db| test_shim_class_type(db, "ExampleEnum", false),
    }

    cast_snapshot! {
        name: type_to_tethered,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| Type::Value(test_value_type(db, "ExampleStruct")),
        to: |db| TetheringStructType {
            source_type: test_value_type(db, "ExampleStruct"),
        },
    }

    cast_snapshot! {
        name: tethered_to_type,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| TetheringStructType {
            source_type: test_value_type(db, "ExampleStruct"),
        },
        to: |db| Type::Value(test_value_type(db, "ExampleStruct")),
    }

    cast_snapshot! {
        name: tethered_to_type_ref,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| TetheringStructType {
            source_type: test_value_type(db, "ExampleStruct"),
        },
        to: |db| test_value_type(db, "ExampleStruct").borrowed(),
    }

    cast_snapshot! {
        name: tethered_to_type_mut,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| TetheringStructType {
            source_type: test_value_type(db, "ExampleStruct"),
        },
        to: |db| test_value_type(db, "ExampleStruct").borrowed_mut(),
    }

    cast_snapshot! {
        name: class_to_tethered,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| test_shim_class_type(db, "ExampleStruct", true),
        to: |db| TetheringStructType {
            source_type: test_value_type(db, "ExampleStruct"),
        },
    }

    cast_snapshot! {
        name: tethered_to_class,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| TetheringStructType {
            source_type: test_value_type(db, "ExampleStruct"),
        },
        to: |db| test_shim_class_type(db, "ExampleStruct", false),
    }

    cast_snapshot! {
        name: rust_identity,
        from: |_db| Type::Value(ValueType::bool),
        to: |_db| Type::Value(ValueType::bool),
    }

    cast_snapshot! {
        name: shim_identity,
        from: |_db| ShimType::bool,
        to: |_db| ShimType::bool,
    }

    cast_snapshot! {
        name: tethered_identity,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| TetheringStructType {
            source_type: test_value_type(db, "ExampleStruct"),
        },
        to: |db| TetheringStructType {
            source_type: test_value_type(db, "ExampleStruct"),
        },
    }

    fn test_value_type<'db>(db: &'db dyn Db, name: &str) -> ValueType<'db> {
        let crate_ = Crate::by(db, TEST_CRATE_NAME).expect("test crate should be lowered");

        crate_
            .root(db)
            .members(db)
            .iter()
            .find_map(|member| match member {
                Member::Struct(struct_) if struct_.name(db) == name => Some((*struct_).into()),
                Member::Enum(enum_) if enum_.name(db) == name => Some((*enum_).into()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("test value type `{name}` should be lowered"))
    }

    fn test_shim_class_type<'db>(db: &'db dyn Db, name: &str, is_borrowed: bool) -> ShimType<'db> {
        let inner = match test_value_type(db, name) {
            ValueType::Struct(struct_) => struct_.shim(db),
            ValueType::Enum(enum_) => enum_.shim(db),
            _ => unreachable!("class cast test requires a struct or enum"),
        };

        ShimType::Class { is_borrowed, inner }
    }
}
