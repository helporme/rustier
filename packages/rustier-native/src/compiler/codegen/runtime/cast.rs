//! Code to cast values from ABI to source types back and forth

mod borrowing;
mod handles;
mod numbers;
mod strings;
mod tethering;

use crate::compiler::{
    codegen::fragment::{FragmentOnce, Printer, frag, fragment},
    db::Db,
    ir::{
        runtime::tethering::RuntimeTethering,
        rust::{Type, TypeKind},
        transport::TransportType,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastType<'db> {
    Tethered(RuntimeTethering<'db>),
    Transport(TransportType),
    Rust(Type<'db>),
}

impl<'db> CastType<'db> {
    fn same_representation(self, db: &'db dyn Db, other: Self) -> bool {
        match (self, other) {
            (Self::Transport(left), Self::Transport(right)) => left == right,
            (Self::Tethered(a), Self::Tethered(b)) => a
                .source(db)
                .src_type(db)
                .same_type_as(db, b.source(db).src_type(db)),
            (Self::Rust(a), Self::Rust(b)) => a.same_type_as(db, b),
            _ => false,
        }
    }
}

impl<'db> From<RuntimeTethering<'db>> for CastType<'db> {
    fn from(value: RuntimeTethering<'db>) -> Self {
        Self::Tethered(value)
    }
}

impl<'db> From<TransportType> for CastType<'db> {
    fn from(value: TransportType) -> Self {
        Self::Transport(value)
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
pub struct RuntimeCastRenderer;

impl<'db> CastRenderer<'db> for RuntimeCastRenderer {
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

            if src.same_representation(db, dst) {
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

                // handles
                (Transport(TransportType::usize), Tethered(target)) => {
                    handles::render_handle_to_tethered(db, target, in_expr, output).apply(printer);
                }
                (Tethered(_), Transport(TransportType::usize)) => {
                    handles::render_tethered_to_handle(in_expr, output).apply(printer);
                }
                (Transport(TransportType::usize), Rust(target)) => {
                    handles::render_handle_to_type(db, target, in_expr, output).apply(printer);
                }
                (Rust(source), Transport(TransportType::usize)) => {
                    handles::render_type_to_handle(db, source, in_expr, output).apply(printer);
                }

                // numbers
                (Transport(TransportType::f64), Rust(target)) => {
                    numbers::render_f64_to_type(db, target, in_expr, output).apply(printer);
                }
                (Rust(source), Transport(TransportType::f64)) => {
                    numbers::render_type_to_f64(db, source, in_expr, output).apply(printer);
                }

                // borrowed primitives
                (Transport(TransportType::Unit), Rust(target))
                    if matches!(target.kind(db), TypeKind::Borrowed(borrowed)
                        if borrowed.inner(db).kind(db) == TypeKind::Unit) =>
                {
                    borrowing::render_owned_to_borrowed(db, target, in_expr, output).apply(printer);
                }
                (Transport(TransportType::bool), Rust(target))
                    if matches!(target.kind(db), TypeKind::Borrowed(borrowed)
                        if borrowed.inner(db).kind(db) == TypeKind::bool) =>
                {
                    borrowing::render_owned_to_borrowed(db, target, in_expr, output).apply(printer);
                }

                // strings
                (Transport(TransportType::String), Rust(target))
                    if matches!(target.kind(db), TypeKind::Borrowed(_)) =>
                {
                    strings::render_string_to_type(db, target, in_expr, output).apply(printer);
                }

                // identity casts
                (Transport(TransportType::Unit), Rust(type_))
                | (Rust(type_), Transport(TransportType::Unit))
                    if type_.kind(db) == TypeKind::Unit =>
                {
                    adapt_to_cast_output(in_expr, output).apply(printer);
                }
                (Transport(TransportType::bool), Rust(type_))
                | (Rust(type_), Transport(TransportType::bool))
                    if type_.kind(db) == TypeKind::bool =>
                {
                    adapt_to_cast_output(in_expr, output).apply(printer);
                }
                (Transport(TransportType::String), Rust(type_))
                | (Rust(type_), Transport(TransportType::String))
                    if type_.kind(db) == TypeKind::String =>
                {
                    adapt_to_cast_output(in_expr, output).apply(printer);
                }
                (Transport(TransportType::JsValueHandle), Rust(type_))
                | (Rust(type_), Transport(TransportType::JsValueHandle))
                    if type_.kind(db) == TypeKind::JsValue =>
                {
                    adapt_to_cast_output(frag!(({ in_expr }).into()), output).apply(printer);
                }

                _ => unreachable!("unsupported runtime cast from `{src:?}` to `{dst:?}`"),
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

    use crate::compiler::{
        codegen::fragment::{FragmentOnce, Printer, frag},
        db::Db,
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
        codegen::fragment::frag,
        db::Db,
        ir::{
            rust::{Member, Rust, Type},
            transport::TransportType,
        },
    };

    macro_rules! cast_snapshot {
        (
            name: $name:ident,
            source: { $($source:tt)* },
            $(fixture: { $($fixture:tt)* },)?
            from: |$from_db:ident| $from:expr,
            to: |$to_db:ident| $to:expr $(,)?
        ) => {
            #[test]
            fn $name() {
                $crate::assert_rust_snap! {
                    source: { $($source)* },
                    expression: |__db, __transport, __printer| {
                        $(let __fixture_db = $crate::test_db! { $($fixture)* };
                          let __db = &__fixture_db;)?
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
                            frag!(
                                fn cast(input: ())~{{
                                    {$crate::compiler::codegen::runtime::cast::CastRenderer::render_stmt(
                                        $crate::compiler::codegen::runtime::cast::RuntimeCastRenderer,
                                        __db,
                                        $crate::compiler::codegen::runtime::cast::Cast {
                                            from: __from,
                                            to: __to,
                                            in_expr: "input",
                                            out_var_name: "output",
                                        },
                                        frag!(consume(output);),
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
                fixture: {
                    pub fn cast_types(
                        char: char, i8: i8, u8: u8, i16: i16, u16: u16,
                        i32: i32, u32: u32, f32: f32, f64: f64,
                        bool: bool, string: String, str: &str,
                    ) {}
                },
                from: |$from_db| $from,
                to: |$to_db| $to,
            }
        };
    }

    macro_rules! number_cast_snapshots {
        (
            value: $value:ident,
            in: $in_name:ident,
            ref_in: $ref_name:ident,
            mut_in: $mut_name:ident,
            out: $out_name:ident $(,)?
        ) => {
            cast_snapshot! {
                name: $in_name,
                from: |_db| TransportType::f64,
                to: |db| test_type(db, stringify!($value)),
            }

            cast_snapshot! {
                name: $ref_name,
                from: |_db| TransportType::f64,
                to: |db| test_type(db, stringify!($value)).borrowed(db),
            }

            cast_snapshot! {
                name: $mut_name,
                from: |_db| TransportType::f64,
                to: |db| test_type(db, stringify!($value)).borrowed_mut(db),
            }

            cast_snapshot! {
                name: $out_name,
                from: |db| test_type(db, stringify!($value)),
                to: |_db| TransportType::f64,
            }
        };
    }

    number_cast_snapshots! {
        value: char,
        in: char_in,
        ref_in: char_ref_in,
        mut_in: char_mut_in,
        out: char_out,
    }

    number_cast_snapshots! {
        value: i8,
        in: i8_in,
        ref_in: i8_ref_in,
        mut_in: i8_mut_in,
        out: i8_out,
    }

    number_cast_snapshots! {
        value: u8,
        in: u8_in,
        ref_in: u8_ref_in,
        mut_in: u8_mut_in,
        out: u8_out,
    }

    number_cast_snapshots! {
        value: i16,
        in: i16_in,
        ref_in: i16_ref_in,
        mut_in: i16_mut_in,
        out: i16_out,
    }

    number_cast_snapshots! {
        value: u16,
        in: u16_in,
        ref_in: u16_ref_in,
        mut_in: u16_mut_in,
        out: u16_out,
    }

    number_cast_snapshots! {
        value: i32,
        in: i32_in,
        ref_in: i32_ref_in,
        mut_in: i32_mut_in,
        out: i32_out,
    }

    number_cast_snapshots! {
        value: u32,
        in: u32_in,
        ref_in: u32_ref_in,
        mut_in: u32_mut_in,
        out: u32_out,
    }

    number_cast_snapshots! {
        value: f32,
        in: f32_in,
        ref_in: f32_ref_in,
        mut_in: f32_mut_in,
        out: f32_out,
    }

    number_cast_snapshots! {
        value: f64,
        in: f64_in,
        ref_in: f64_ref_in,
        mut_in: f64_mut_in,
        out: f64_out,
    }

    cast_snapshot! {
        name: bool_in,
        from: |_db| TransportType::bool,
        to: |db| test_type(db, "bool"),
    }

    cast_snapshot! {
        name: bool_ref_in,
        from: |_db| TransportType::bool,
        to: |db| test_type(db, "bool").borrowed(db),
    }

    cast_snapshot! {
        name: bool_mut_in,
        from: |_db| TransportType::bool,
        to: |db| test_type(db, "bool").borrowed_mut(db),
    }

    cast_snapshot! {
        name: bool_out,
        from: |db| test_type(db, "bool"),
        to: |_db| TransportType::bool,
    }

    cast_snapshot! {
        name: string_in,
        from: |_db| TransportType::String,
        to: |db| test_type(db, "string"),
    }

    cast_snapshot! {
        name: string_ref_in,
        from: |_db| TransportType::String,
        to: |db| test_type(db, "string").borrowed(db),
    }

    cast_snapshot! {
        name: string_mut_in,
        from: |_db| TransportType::String,
        to: |db| test_type(db, "string").borrowed_mut(db),
    }

    cast_snapshot! {
        name: str_ref_in,
        from: |_db| TransportType::String,
        to: |db| test_type(db, "str").borrowed(db),
    }

    cast_snapshot! {
        name: str_mut_in,
        from: |_db| TransportType::String,
        to: |db| test_type(db, "str").borrowed_mut(db),
    }

    cast_snapshot! {
        name: string_out,
        from: |db| test_type(db, "string"),
        to: |_db| TransportType::String,
    }

    cast_snapshot! {
        name: struct_in,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |_db| TransportType::usize,
        to: |db| test_type(db, "ExampleStruct"),
    }

    cast_snapshot! {
        name: struct_ref_in,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |_db| TransportType::usize,
        to: |db| test_type(db, "ExampleStruct").borrowed(db),
    }

    cast_snapshot! {
        name: struct_mut_in,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |_db| TransportType::usize,
        to: |db| test_type(db, "ExampleStruct").borrowed_mut(db),
    }

    cast_snapshot! {
        name: struct_out,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| test_type(db, "ExampleStruct"),
        to: |_db| TransportType::usize,
    }

    cast_snapshot! {
        name: enum_in,
        source: {
            #[derive(Clone)]
            pub enum ExampleEnum {
                Variant,
            }
        },
        from: |_db| TransportType::usize,
        to: |db| test_type(db, "ExampleEnum"),
    }

    cast_snapshot! {
        name: enum_ref_in,
        source: {
            #[derive(Clone)]
            pub enum ExampleEnum {
                Variant,
            }
        },
        from: |_db| TransportType::usize,
        to: |db| test_type(db, "ExampleEnum").borrowed(db),
    }

    cast_snapshot! {
        name: enum_mut_in,
        source: {
            #[derive(Clone)]
            pub enum ExampleEnum {
                Variant,
            }
        },
        from: |_db| TransportType::usize,
        to: |db| test_type(db, "ExampleEnum").borrowed_mut(db),
    }

    cast_snapshot! {
        name: enum_out,
        source: {
            #[derive(Clone)]
            pub enum ExampleEnum {
                Variant,
            }
        },
        from: |db| test_type(db, "ExampleEnum"),
        to: |_db| TransportType::usize,
    }

    cast_snapshot! {
        name: type_to_tethered,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| test_type(db, "ExampleStruct"),
        to: |db| test_type(db, "ExampleStruct").tethering(db).unwrap(),
    }

    cast_snapshot! {
        name: tethered_to_type,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| test_type(db, "ExampleStruct").tethering(db).unwrap(),
        to: |db| test_type(db, "ExampleStruct"),
    }

    cast_snapshot! {
        name: tethered_to_type_ref,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| test_type(db, "ExampleStruct").tethering(db).unwrap(),
        to: |db| test_type(db, "ExampleStruct").borrowed(db),
    }

    cast_snapshot! {
        name: tethered_to_type_mut,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| test_type(db, "ExampleStruct").tethering(db).unwrap(),
        to: |db| test_type(db, "ExampleStruct").borrowed_mut(db),
    }

    cast_snapshot! {
        name: class_to_tethered,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |_db| TransportType::usize,
        to: |db| test_type(db, "ExampleStruct").tethering(db).unwrap(),
    }

    cast_snapshot! {
        name: tethered_to_class,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| test_type(db, "ExampleStruct").tethering(db).unwrap(),
        to: |_db| TransportType::usize,
    }

    cast_snapshot! {
        name: rust_identity,
        from: |db| test_type(db, "bool"),
        to: |db| test_type(db, "bool"),
    }

    cast_snapshot! {
        name: abi_identity,
        from: |_db| TransportType::bool,
        to: |_db| TransportType::bool,
    }

    cast_snapshot! {
        name: tethered_identity,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| test_type(db, "ExampleStruct").tethering(db).unwrap(),
        to: |db| test_type(db, "ExampleStruct").tethering(db).unwrap(),
    }

    #[test]
    fn identity_casts_ignore_origins_but_preserve_mutability() {
        use super::{CastRenderer, CastType, RuntimeCastRenderer};
        use crate::compiler::codegen::fragment::FragmentPrint as _;

        let db = crate::test_db! {
            #[derive(Clone)]
            pub struct Item;
            pub fn values(first: &i32, second: &i32, mutable: &mut i32, item: Item, other: Item) {}
        };
        ra_ap_hir::attach_db(&db, || {
            let Member::Function(function) = Rust::get(&db).crates(&db)[0]
                .root(&db)
                .members(&db)
                .iter()
                .find(|member| matches!(member, Member::Function(_)))
                .unwrap()
            else {
                unreachable!()
            };
            let params = function.declaration(&db).params(&db);
            let type_at = |index: usize| *params[index].type_(&db);
            let first = CastType::Rust(type_at(0));
            let second = CastType::Rust(type_at(1));
            assert_ne!(first, second);
            assert!(!first.same_representation(&db, CastType::Rust(type_at(2))));
            let renderer = RuntimeCastRenderer;

            assert_eq!(
                renderer.render_expr(&db, first, second, "input").print(),
                "input"
            );
            let first = type_at(3).tethering(&db).unwrap();
            let second = type_at(4).tethering(&db).unwrap();
            assert_ne!(first, second);
            assert_eq!(
                renderer.render_expr(&db, first, second, "input").print(),
                "input"
            );
        });
    }

    fn test_type<'db>(db: &'db dyn Db, name: &str) -> Type<'db> {
        let crate_ = Rust::get(db)
            .crates(db)
            .first()
            .expect("test crate should be lowered");

        crate_
            .root(db)
            .members(db)
            .iter()
            .find_map(|member| match member {
                Member::Struct(struct_) if struct_.segment(db).name(db) == name => {
                    struct_.as_mono_type(db)
                }
                Member::Enum(enum_) if enum_.segment(db).name(db) == name => enum_.as_mono_type(db),
                Member::Function(function) if function.declaration(db).name(db) == "cast_types" => {
                    function
                        .declaration(db)
                        .params(db)
                        .iter()
                        .find(|param| param.name(db) == name)
                        .map(|param| param.type_(db).deborrow(db))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("test value type `{name}` should be lowered"))
    }
}
