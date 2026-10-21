use std::{fmt::Write as _, path::Path};

use crate::compiler::{
    codegen::fragment::{FragmentOnce, FragmentPrint, comma_list, doubles, frag, fragment},
    db::Db,
    ir::transport::{Transport, TransportFunction, TransportReturn, TransportType},
};

pub const NAPI_LOADER: &str = include_str!("napi-loader.mjs");

pub fn render_wasm_node_loader(bindings_file: &str, wasm_file: &str) -> String {
    let bindings = || doubles(("./", bindings_file));
    let binary = doubles(("./", wasm_file));

    frag!(
        import ~{{ readFileSync }}~ from "node:fs";@
        import ~{{ initSync }}~ from ~{bindings()};@
        export * from ~{bindings()};@@

        export const __rustier_wasm_exports = initSync({{
            module: readFileSync(new URL({binary}, import.meta.url))
        }});
    )
    .print()
}

/// How the generated WebAssembly module is initialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum WasmLoading {
    Fetch,
    ModuleNode,
    Module,
}

#[derive(Clone, Copy)]
pub enum TransportJsTarget<'a> {
    Native {
        path: &'a str,
    },
    Wasm {
        loading: WasmLoading,
        path: &'a str,
        wasm_path: &'a str,
    },
}

impl<'db> Transport<'db> {
    pub fn render_js(self, db: &'db dyn Db, target: TransportJsTarget<'_>) -> String {
        let symbols = self.symbols(db);

        target.render_js_with(fragment(move |p| {
            for symbol in symbols {
                frag!(export~const~{symbol}~=~abi.{symbol};@).apply(p);
            }
        }))
    }
}

impl TransportJsTarget<'_> {
    pub fn render_js_with(self, body: impl FragmentOnce) -> String {
        let mut js = fragment(move |p| {
            match self {
                TransportJsTarget::Native { path } => {
                    let path = fragment(move |p| write!(p, "{path:?}").unwrap());

                    frag!(
                        import ~{{ native as abi }}~ from ~{path};@
                    )
                    .apply(p);
                }
                TransportJsTarget::Wasm {
                    loading,
                    path,
                    wasm_path,
                } => {
                    let host_path = Path::new(path)
                        .with_file_name(rustier_ext::transport::WASM_RUNTIME_FILENAME);
                    let host_path =
                        fragment(move |p| write!(p, "{:?}", host_path.to_string_lossy()).unwrap());
                    let path = fragment(move |p| write!(p, "{path:?}").unwrap());

                    match loading {
                        WasmLoading::ModuleNode => frag!(
                            import * as abi from ~{path};@
                            const __wasm = abi.__rustier_wasm_exports;@
                        )
                        .apply(p),
                        WasmLoading::Fetch => frag!(
                            import init, * as abi from ~{path};@
                            const __wasm = await init();@
                        )
                        .apply(p),
                        WasmLoading::Module => {
                            let wasm_path = fragment(move |p| write!(p, "{wasm_path:?}").unwrap());

                            frag!(
                                import wasmModule from ~{wasm_path};@
                                import init, * as abi from ~{path};@
                                const __wasm = await init({{ module_or_path: wasmModule }});@
                            )
                            .apply(p);
                        }
                    }

                    frag!(
                        import ~{{
                            __rustier_js_init_memory,
                            __rustier_js_insert,
                            __rustier_js_take,
                            __rustier_js_drop_ref
                        }}~ from ~{host_path};@@
                        __rustier_js_init_memory(__wasm.memory);@@
                    )
                    .apply(p);
                }
            }

            body.apply(p);
        })
        .print();

        js.push('\n');

        js
    }

    pub fn render_call<'db, Body>(
        self,
        db: &'db dyn Db,
        function: TransportFunction<'db>,
        body: impl Fn(TransportJsReturn) -> Body + 'db,
    ) -> impl FragmentOnce + 'db
    where
        Body: FragmentOnce + 'db,
    {
        let wasm = matches!(self, Self::Wasm { .. });
        let types = function.params(db);
        let symbol = function.symbol(db);
        let args = comma_list(types.iter().enumerate().map(move |(index, type_)| {
            fragment(move |p| {
                if wasm && *type_ == TransportType::JsValueHandle {
                    frag!(__js_ { index }).apply(p);
                } else {
                    frag!(_{index}).apply(p);
                }
            })
        }));
        let ret_type = function.ret_type(db);
        let call = function.render_js_with(db, frag!(abi.{symbol}({args})), move |value| {
            let decode = wasm && value.type_ == TransportType::JsValueHandle;
            let union = matches!(ret_type, TransportReturn::Union(_));
            let output = body(if decode && !union {
                TransportJsReturn {
                    expr: "__js_value",
                    whole_expr: "__js_value",
                    ..value
                }
            } else {
                value
            });

            fragment(move |p| {
                if decode {
                    if union {
                        frag!(__abi_return[1] = __rustier_js_take({value.expr});@).apply(p);
                    } else {
                        frag!(const __js_value = __rustier_js_take({value.expr});@).apply(p);
                    }
                }

                output.apply(p);
            })
        });

        fragment(move |p| {
            for (index, type_) in types.iter().enumerate() {
                if wasm && *type_ == TransportType::JsValueHandle {
                    frag!(
                        const __js_{index} = __rustier_js_insert(_{index});@
                        try ~{"{"}@
                    )
                    .apply(p);
                }
            }

            call.apply(p);

            for (index, type_) in types.iter().enumerate().rev() {
                if wasm && *type_ == TransportType::JsValueHandle {
                    frag!(
                        @{"}"}~ finally ~{"{"}@
                        __rustier_js_drop_ref(__js_{index});@
                        {"}"}@
                    )
                    .apply(p);
                }
            }
        })
    }
}

impl<'db> TransportFunction<'db> {
    pub fn render_js_with<Body>(
        self,
        db: &'db dyn Db,
        call: impl FragmentOnce + 'db,
        body: impl Fn(TransportJsReturn) -> Body + 'db,
    ) -> impl FragmentOnce + 'db
    where
        Body: FragmentOnce + 'db,
    {
        self.ret_type(db).render_js_with(db, call, body)
    }
}

impl<'db> TransportReturn<'db> {
    pub(crate) fn render_js_with<Body>(
        self,
        db: &'db dyn Db,
        call: impl FragmentOnce + 'db,
        body: impl Fn(TransportJsReturn) -> Body + 'db,
    ) -> impl FragmentOnce + 'db
    where
        Body: FragmentOnce + 'db,
    {
        let branches = fragment(move |p| match self {
            TransportReturn::Value(type_) => body(TransportJsReturn {
                index: 0,
                type_,
                expr: "__abi_return",
                whole_expr: "__abi_return",
            })
            .apply(p),
            TransportReturn::Union(union) => {
                let cases = fragment(move |p| {
                    for (index, type_) in union.variants(db).iter().copied().enumerate() {
                        let branch = body(TransportJsReturn {
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
pub struct TransportJsReturn {
    pub index: usize,
    pub type_: TransportType,
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
        ir::{rust::Rust, transport::TransportUnion},
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
        let body = TransportReturn::Value(TransportType::f64).render_js_with(
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
        let union = TransportUnion::new(&db, vec![TransportType::f64, TransportType::String]);
        let body = TransportReturn::Union(union).render_js_with(
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
            let abi = Rust::get(&db).facade_complete(&db).transport(&db);

            for (name, target) in [
                (
                    "native_module",
                    TransportJsTarget::Native {
                        path: "./native.js",
                    },
                ),
                (
                    "wasm_module",
                    TransportJsTarget::Wasm {
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
