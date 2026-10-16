//! Code to cast values between facade and ABI representations.

mod classes;
mod primitives;

use crate::compiler::{
    codegen::fragment::{FragmentOnce, Printer, frag, fragment},
    db::Db,
    ir::{abi::AbiType, facade::type_js::FacadeTypeJs},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastType<'db> {
    Facade(FacadeTypeJs<'db>),
    Abi(AbiType),
}

impl<'db> From<FacadeTypeJs<'db>> for CastType<'db> {
    fn from(value: FacadeTypeJs<'db>) -> Self {
        Self::Facade(value)
    }
}

impl<'db> From<AbiType> for CastType<'db> {
    fn from(value: AbiType) -> Self {
        Self::Abi(value)
    }
}

pub struct Cast<From, To, InExpr, OutVar> {
    pub from: From,
    pub to: To,
    pub in_expr: InExpr,
    pub out_var_name: OutVar,
}

pub trait CastRenderer<'db>: Copy + 'db {
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
        Continue: FragmentOnce + 'db;

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
        let Cast {
            from,
            to,
            in_expr,
            out_var_name,
        } = cast;
        let src = from.into();
        let dst = to.into();

        fragment(move |printer| {
            use CastType::*;

            match (src, dst) {
                (Abi(AbiType::Unit), Facade(FacadeTypeJs::Unit))
                | (Abi(AbiType::bool), Facade(FacadeTypeJs::boolean))
                | (Abi(AbiType::String), Facade(FacadeTypeJs::string))
                | (Abi(AbiType::f64), Facade(FacadeTypeJs::number))
                | (Abi(AbiType::Unit), Abi(AbiType::Unit))
                | (Abi(AbiType::bool), Abi(AbiType::bool))
                | (Abi(AbiType::String), Abi(AbiType::String))
                | (Abi(AbiType::f64), Abi(AbiType::f64))
                | (Abi(AbiType::usize), Abi(AbiType::usize)) => {
                    frag!(
                        const ~{out_var_name}~=~{in_expr};@
                        {continue_with}
                    )
                    .apply(printer);
                }
                (Facade(FacadeTypeJs::Unit), Abi(AbiType::Unit))
                | (Facade(FacadeTypeJs::Unit), Facade(FacadeTypeJs::Unit)) => {
                    primitives::render_js_checked_identity(
                        "undefined",
                        in_expr,
                        out_var_name,
                        continue_with,
                    )
                    .apply(printer);
                }
                // classes
                (Facade(FacadeTypeJs::Class(source)), Abi(AbiType::usize)) => {
                    classes::render_facade_class_to_abi(
                        db,
                        source,
                        in_expr,
                        out_var_name,
                        continue_with,
                    )
                    .apply(printer);
                }
                (Abi(AbiType::usize), Facade(FacadeTypeJs::Class(target))) => {
                    classes::render_abi_handle_to_facade(
                        db,
                        target,
                        in_expr,
                        out_var_name,
                        continue_with,
                    )
                    .apply(printer);
                }
                // boolean
                (Facade(FacadeTypeJs::boolean), Abi(AbiType::bool))
                | (Facade(FacadeTypeJs::boolean), Facade(FacadeTypeJs::boolean)) => {
                    primitives::render_bool_cast(in_expr, out_var_name, continue_with)
                        .apply(printer);
                }

                // string
                (Facade(FacadeTypeJs::string), Abi(AbiType::String))
                | (Facade(FacadeTypeJs::string), Facade(FacadeTypeJs::string)) => {
                    primitives::render_string_cast(in_expr, out_var_name, continue_with)
                        .apply(printer);
                }

                // number
                (Facade(FacadeTypeJs::number), Abi(AbiType::f64))
                | (Facade(FacadeTypeJs::number), Facade(FacadeTypeJs::number)) => {
                    primitives::render_number_cast(in_expr, out_var_name, continue_with)
                        .apply(printer);
                }

                _ => unreachable!("unsupported facade cast from `{src:?}` to `{dst:?}`"),
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

#[cfg(test)]
pub mod test {
    use std::fmt::{Debug, Write as _};

    use crate::compiler::{
        codegen::fragment::{FragmentOnce, Printer, frag},
        db::Db,
    };

    use super::{Cast, CastRenderer, CastType};

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
            let Cast {
                from,
                to,
                in_expr,
                out_var_name,
            } = cast;
            let from = DebugString {
                db,
                value: from.into(),
            };
            let to = DebugString {
                db,
                value: to.into(),
            };
            let expr = frag!(cast.from({ from }).to({ to }).an({ in_expr }));

            frag!({{
                const ~{out_var_name}~ = ~{expr};@
                {continue_with}
            }})
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::{
        codegen::fragment::frag,
        db::Db,
        ir::{
            abi::AbiType,
            facade::{FacadeClass, FacadeMember, type_js::FacadeTypeJs},
            rust::Rust,
        },
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
                $crate::assert_js_snap! {
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
                            frag!(
                                function cast(input)~{{
                                    {$crate::compiler::codegen::js::cast::CastRenderer::render_stmt(
                                        $crate::compiler::codegen::js::cast::Renderer,
                                        __db,
                                        $crate::compiler::codegen::js::cast::Cast {
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
                from: |$from_db| $from,
                to: |$to_db| $to,
            }
        };
    }

    cast_snapshot! {
        name: boolean_to_abi,
        from: |_db| FacadeTypeJs::boolean,
        to: |_db| AbiType::bool,
    }

    cast_snapshot! {
        name: boolean_to_facade,
        from: |_db| AbiType::bool,
        to: |_db| FacadeTypeJs::boolean,
    }

    cast_snapshot! {
        name: string_to_abi,
        from: |_db| FacadeTypeJs::string,
        to: |_db| AbiType::String,
    }

    cast_snapshot! {
        name: string_to_facade,
        from: |_db| AbiType::String,
        to: |_db| FacadeTypeJs::string,
    }

    cast_snapshot! {
        name: number_to_abi,
        from: |_db| FacadeTypeJs::number,
        to: |_db| AbiType::f64,
    }

    cast_snapshot! {
        name: number_to_facade,
        from: |_db| AbiType::f64,
        to: |_db| FacadeTypeJs::number,
    }

    cast_snapshot! {
        name: class_to_abi,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |db| FacadeTypeJs::Class(test_facade_class(db)),
        to: |_db| AbiType::usize,
    }

    cast_snapshot! {
        name: class_to_facade,
        source: {
            #[derive(Clone)]
            pub struct ExampleStruct;
        },
        from: |_db| AbiType::usize,
        to: |db| FacadeTypeJs::Class(test_facade_class(db)),
    }

    cast_snapshot! {
        name: facade_identity,
        from: |_db| FacadeTypeJs::number,
        to: |_db| FacadeTypeJs::number,
    }

    cast_snapshot! {
        name: abi_identity,
        from: |_db| AbiType::f64,
        to: |_db| AbiType::f64,
    }

    fn test_facade_class<'db>(db: &'db dyn Db) -> FacadeClass<'db> {
        let crate_ = Rust::get(db)
            .facade_complete(db)
            .crates(db)
            .next()
            .expect("test crate should be lowered");
        let root = crate_.root(db);
        let mut classes = root
            .members(db)
            .iter()
            .copied()
            .filter_map(|member| match member {
                FacadeMember::Class(class) => Some(class),
                _ => None,
            });
        let (Some(class), None) = (classes.next(), classes.next()) else {
            panic!("test crate root should contain exactly one facade class");
        };

        class
    }
}
