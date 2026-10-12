use std::fmt::Write as _;

use crate::compiler::{
    codegen::fragment::{FragmentOnce, FragmentPrint, frag, fragment},
    db::Db,
    ir::abi::Abi,
};

pub const NAPI_LOADER: &str = include_str!("ffi/napi-loader.mjs");

/// How the generated WebAssembly module is initialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum WasmLoading {
    Fetch,
    ModuleNode,
    Module,
}

pub enum FfiTarget<'a> {
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
    pub fn render(self, db: &'db dyn Db, target: FfiTarget<'_>) -> String {
        let symbols = self.symbols(db);

        let mut js = fragment(move |p| {
            match target {
                FfiTarget::Native { path } => {
                    let path = fragment(move |p| write!(p, "{path:?}").unwrap());

                    frag!(
                        { "import { native as ffi } from " }{path}{ ";\n" }
                    )
                    .apply(p);
                }
                FfiTarget::Wasm {
                    loading,
                    path,
                    wasm_path,
                } => {
                    let path = || fragment(move |p| write!(p, "{path:?}").unwrap());

                    match loading {
                        WasmLoading::ModuleNode => frag!(
                            { "import * as ffi from " }{path()}{ ";\n" }
                        )
                        .apply(p),
                        WasmLoading::Fetch => frag!(
                            { "import init, * as ffi from " }{path()}
                            { ";\nawait init();\n" }
                        )
                        .apply(p),
                        WasmLoading::Module => {
                            let wasm_path = fragment(move |p| write!(p, "{wasm_path:?}").unwrap());

                            frag!(
                                { "import wasmModule from " }{wasm_path}
                                { ";\nimport init, * as ffi from " }{path()}
                                { ";\nawait init({ module_or_path: wasmModule });\n" }
                            )
                            .apply(p);
                        }
                    }
                }
            }

            for symbol in symbols {
                frag!(export~const~{symbol}~=~ffi.{symbol};@).apply(p);
            }
        })
        .print();

        js.push('\n');

        js
    }
}
