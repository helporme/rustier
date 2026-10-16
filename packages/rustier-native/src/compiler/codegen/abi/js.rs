use std::fmt::Write as _;

use crate::compiler::{
    codegen::fragment::{FragmentOnce, FragmentPrint, frag, fragment},
    db::Db,
    ir::abi::{Abi, AbiFunction, AbiReturn, AbiType},
};

pub const NAPI_LOADER: &str = include_str!("napi-loader.mjs");

/// How the generated WebAssembly module is initialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum WasmLoading {
    Fetch,
    ModuleNode,
    Module,
}

pub enum AbiJsTarget<'a> {
    Native {
        path: &'a str,
    },
    Wasm {
        loading: WasmLoading,
        path: &'a str,
        wasm_path: &'a str,
    },
}

impl<'db> Abi<'db> {
    pub fn render_js(self, db: &'db dyn Db, target: AbiJsTarget<'_>) -> String {
        let symbols = self.symbols(db);

        target.render_js_with(fragment(move |p| {
            for symbol in symbols {
                frag!(export~const~{symbol}~=~abi.{symbol};@).apply(p);
            }
        }))
    }
}

impl AbiJsTarget<'_> {
    pub fn render_js_with(self, body: impl FragmentOnce) -> String {
        let mut js = fragment(move |p| {
            match self {
                AbiJsTarget::Native { path } => {
                    let path = fragment(move |p| write!(p, "{path:?}").unwrap());

                    frag!(
                        import ~{{ native as abi }}~ from ~{path};@
                    )
                    .apply(p);
                }
                AbiJsTarget::Wasm {
                    loading,
                    path,
                    wasm_path,
                } => {
                    let path = fragment(move |p| write!(p, "{path:?}").unwrap());

                    match loading {
                        WasmLoading::ModuleNode => frag!(
                            import * as abi from ~{path};@
                        )
                        .apply(p),
                        WasmLoading::Fetch => frag!(
                            import init, * as abi from ~{path};@
                            await init();@
                        )
                        .apply(p),
                        WasmLoading::Module => {
                            let wasm_path = fragment(move |p| write!(p, "{wasm_path:?}").unwrap());

                            frag!(
                                import wasmModule from ~{wasm_path};@
                                import init, * as abi from ~{path};@
                                await init({{ module_or_path: wasmModule }});@
                            )
                            .apply(p);
                        }
                    }
                }
            }

            body.apply(p);
        })
        .print();

        js.push('\n');

        js
    }
}

impl<'db> AbiFunction<'db> {
    pub fn render_js_with<Body>(
        self,
        db: &'db dyn Db,
        call: impl FragmentOnce + 'db,
        body: impl Fn(AbiJsReturn) -> Body + 'db,
    ) -> impl FragmentOnce + 'db
    where
        Body: FragmentOnce + 'db,
    {
        self.ret_type(db).render_js_with(db, call, body)
    }
}

impl<'db> AbiReturn<'db> {
    pub(crate) fn render_js_with<Body>(
        self,
        db: &'db dyn Db,
        call: impl FragmentOnce + 'db,
        body: impl Fn(AbiJsReturn) -> Body + 'db,
    ) -> impl FragmentOnce + 'db
    where
        Body: FragmentOnce + 'db,
    {
        let branches = fragment(move |p| match self {
            AbiReturn::Value(type_) => body(AbiJsReturn {
                index: 0,
                type_,
                expr: "__abi_return",
                whole_expr: "__abi_return",
            })
            .apply(p),
            AbiReturn::Union(union) => {
                let cases = fragment(move |p| {
                    for (index, type_) in union.variants(db).iter().copied().enumerate() {
                        let branch = body(AbiJsReturn {
                            index,
                            type_,
                            expr: "__abi_return[1]",
                            whole_expr: "__abi_return",
                        });

                        frag!(
                            case ~{index}: {{
                                {branch}@
                                break;
                            }}@
                        )
                        .apply(p);
                    }
                });

                frag!(
                    switch (__abi_return[0]) {{
                        {cases}
                    }}
                )
                .apply(p);
            }
        });

        frag!({{
            const __abi_return = {call};@
            {branches}
        }})
    }
}

#[derive(Clone, Copy)]
pub struct AbiJsReturn {
    pub index: usize,
    pub type_: AbiType,
    pub expr: &'static str,
    pub whole_expr: &'static str,
}

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    use super::*;
    use crate::compiler::{
        db::RootDatabase,
        ir::{abi::AbiUnion, rust::Rust},
    };

    fn snapshot(name: &str, compiled: &str) {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, compiled, SourceType::mjs()).parse();

        if !parsed.diagnostics.is_empty() {
            panic!(
                "invalid generated JavaScript:\n{:#?}\n{compiled}",
                parsed.diagnostics
            );
        }

        let snapshot = ("```js\n", compiled.trim_end(), "\n```\n").print();

        insta::with_settings!({
            snapshot_path => "_snapshots/js",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!(name, snapshot);
        });
    }

    #[test]
    fn scalar_return() {
        let db = RootDatabase::default();
        let body = AbiReturn::Value(AbiType::f64).render_js_with(
            &db,
            frag!(abi.call()),
            |value| frag!(return ~{value.expr};),
        );
        let compiled = frag!(export function call() {{ {body} }}).print();

        snapshot("scalar_return", &compiled);
    }

    #[test]
    fn union_returns() {
        let db = RootDatabase::default();
        let union = AbiUnion::new(&db, vec![AbiType::f64, AbiType::String]);
        let body = AbiReturn::Union(union).render_js_with(
            &db,
            frag!(abi.call()),
            |value| frag!(consume({value.index}, {value.expr});),
        );
        let compiled = frag!(export function call() {{ {body} }}).print();

        snapshot("union_returns", &compiled);
    }

    #[test]
    fn module_loading() {
        let db = crate::test_db! {
            pub fn echo(value: bool) -> bool {
                value
            }
        };

        ra_ap_hir::attach_db(&db, || {
            let abi = Rust::get(&db).facade_complete(&db).abi(&db);

            for (name, target) in [
                (
                    "native_module",
                    AbiJsTarget::Native {
                        path: "./native.js",
                    },
                ),
                (
                    "wasm_module",
                    AbiJsTarget::Wasm {
                        loading: WasmLoading::Module,
                        path: "./bindings.js",
                        wasm_path: "./bindings_bg.wasm",
                    },
                ),
            ] {
                snapshot(name, &abi.render_js(&db, target));
            }
        });
    }
}
