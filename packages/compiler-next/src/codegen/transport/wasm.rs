//! Wasm transport backend.

use crate::{
    codegen::fragment::{FragmentOnce, FragmentPrint, frag, fragment},
    db::Db,
    ir::transport::{TransportFunction, TransportTypeJs, TransportTypeRs},
};

/// WebAssembly transport configuration.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TransportBackendWasm {
    /// Whether the Wasm module uses 64-bit pointers and memory.
    pub mem64: bool,
    pub import: WasmImport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WasmImport {
    /// Fetch and asynchronously instantiate a Wasm binary.
    Fetch,
    /// Read a local Wasm binary with Node.js and instantiate synchronously.
    NodeJs,
    /// Import a bundler-provided `WebAssembly.Module` as the default export.
    Module,
}

pub(super) fn render_transport_preamble_rs(wasm: &TransportBackendWasm) -> impl FragmentOnce {
    let env_type = if wasm.mem64 { "u64" } else { "u32" };

    frag!(
        use ::aux_next::transport::{{JsEnvHandle as __RustierEnvHandle, JsValueHandle as __RustierValueHandle}};@
        type __RustierEnvRaw = {env_type};@
    )
}

pub(super) fn render_transport_preamble_js(
    wasm: &TransportBackendWasm,
    bin_file_path: &String,
) -> impl FragmentOnce {
    let wasm_path = format!("\"{bin_file_path}\"");
    let values = include_str!("wasm/runtime.mjs");

    let loading = fragment(move |p| {
        match wasm.import {
            WasmImport::Fetch => frag!(
                const {{ instance: __rustier_instance }} = await WebAssembly.instantiate(
                    await (await fetch(new URL({wasm_path}, import.meta.url))).arrayBuffer(),
                    __rustier_imports
                );@
            ).apply(p),

            WasmImport::NodeJs => frag!(
                import {{ readFileSync as __rustier_read_file }} from "node:fs";@
                const __rustier_instance = new WebAssembly.Instance(
                    new WebAssembly.Module(__rustier_read_file(new URL({wasm_path}, import.meta.url))),
                    __rustier_imports
                );@
            ).apply(p),

            WasmImport::Module => frag!(
                import __rustier_module from ~{wasm_path};@
                const __rustier_instance = new WebAssembly.Instance(__rustier_module, __rustier_imports);@
            ).apply(p),
        }
    });

    // The import-module key must match the #[link] name in aux-next/src/transport/wasm.rs.
    frag!(
        const __rustier_values = {values};@
        const __rustier_imports = {{ "./rustier-wasm-rt.mjs": __rustier_values }};@
        {loading}
        const __rustier_exports = __rustier_instance.exports;@
        __rustier_values.__rustier_js_init_memory(__rustier_exports.memory);@
    )
}

impl TransportBackendWasm {
    pub(super) fn render_rs_adapter_with_body<'db>(
        &self,
        function: TransportFunction<'db>,
        db: &'db dyn Db,
        render_body: impl FnOnce(&str, &[String]) -> String,
    ) -> String {
        let name = function.symbol(db);
        let arguments: Vec<_> = function
            .inputs(db)
            .iter()
            .enumerate()
            .map(|(index, _)| format!("__rustier_handle_{index}"))
            .collect();
        let body = render_body("__rustier_env_handle", &arguments);
        let params = fragment(|p| {
            for (index, type_) in function.inputs(db).iter().enumerate() {
                match type_ {
                    TransportTypeRs::Void => {}
                    TransportTypeRs::Bool => {
                        frag!(, ~__rustier_raw_{index}: bool).apply(p);
                    }
                    TransportTypeRs::F64 => {
                        frag!(, ~__rustier_raw_{index}: f64).apply(p);
                    }
                    TransportTypeRs::JsValueHandle | TransportTypeRs::String => {
                        frag!(, ~__rustier_raw_{index}: u32).apply(p);
                    }
                }
            }
        });
        let conversions = fragment(|p| {
            for (index, type_) in function.inputs(db).iter().enumerate() {
                match type_ {
                    TransportTypeRs::String => {
                        frag!(
                            let __rustier_handle_{index} = unsafe {{
                                __RustierValueHandle::clone_from_raw(__rustier_raw_{index})
                            }}.to_string().expect("Expected a JavaScript string");@
                        )
                        .apply(p);
                    }
                    TransportTypeRs::Void => {
                        frag!(let __rustier_handle_{index} = ();@).apply(p);
                    }
                    TransportTypeRs::F64 | TransportTypeRs::Bool => {
                        frag!(let __rustier_handle_{index} = __rustier_raw_{index};@).apply(p);
                    }
                    TransportTypeRs::JsValueHandle => {
                        frag!(
                            let __rustier_handle_{index} = unsafe {{
                                __RustierValueHandle::clone_from_raw(
                                    __rustier_raw_{index}
                                )
                            }};@
                        )
                        .apply(p);
                    }
                }
            }
        });
        let export = match function.ret_type(db) {
            TransportTypeRs::String => {
                "__rustier_env_handle.new_string(&__rustier_output).into_raw()"
            }
            TransportTypeRs::Void => "()",
            TransportTypeRs::F64 | TransportTypeRs::Bool => "__rustier_output",
            TransportTypeRs::JsValueHandle => "__rustier_output.into_raw()",
        };
        let ret = match function.ret_type(db) {
            TransportTypeRs::Bool => "bool",
            TransportTypeRs::Void => "()",
            TransportTypeRs::F64 => "f64",
            TransportTypeRs::JsValueHandle | TransportTypeRs::String => "u32",
        };

        frag!(
            #[unsafe(no_mangle)]@
            pub unsafe extern "C" fn ~{name}(
                __rustier_env_raw: __RustierEnvRaw{params}
            ) -> {ret} ~{"{\n"}
                let __rustier_env_handle = unsafe {{
                    __RustierEnvHandle::from_raw(__rustier_env_raw as usize)
                }};@
                {conversions}@
                let __rustier_output = {"{\n"}{body}{"\n};"}@@
                {export}
            {"\n}"}
        )
        .print()
    }

    pub(super) fn render_js_ffi_call_stmt<'db>(
        &self,
        function: TransportFunction<'db>,
        db: &'db dyn Db,
        arguments: &[String],
        result_var: &str,
    ) -> String {
        let env_raw = if self.mem64 { "0n" } else { "0" };
        let name = function.symbol(db);
        let inputs = fragment(|p| {
            for (type_, argument) in function.inputs(db).iter().zip(arguments) {
                match type_ {
                    TransportTypeRs::Void | TransportTypeRs::F64 | TransportTypeRs::Bool => {}
                    TransportTypeRs::JsValueHandle | TransportTypeRs::String => {
                        frag!(handles.push(__rustier_values.__rustier_js_insert({argument}));@)
                            .apply(p);
                    }
                }
            }
        });
        let result = fragment(|p| match function.ret_type(db).js() {
            TransportTypeJs::Boolean => frag!(Boolean(result)).apply(p),
            TransportTypeJs::Undefined => frag!(undefined).apply(p),
            TransportTypeJs::Number => frag!(result).apply(p),
            TransportTypeJs::Any | TransportTypeJs::String => {
                frag!(__rustier_values.__rustier_js_take(result)).apply(p)
            }
        });
        let mut handle_index = 0;
        let call_arguments = fragment(|p| {
            for (type_, argument) in function.inputs(db).iter().zip(arguments) {
                match type_ {
                    TransportTypeRs::Void => {}
                    TransportTypeRs::F64 | TransportTypeRs::Bool => frag!(, ~{argument}).apply(p),
                    TransportTypeRs::JsValueHandle | TransportTypeRs::String => {
                        frag!(, ~handles[{handle_index}]).apply(p);
                        handle_index += 1;
                    }
                }
            }
        });

        let call = frag!(
            const result = __rustier_exports.{name}({env_raw}{call_arguments});@
            {result_var} = {result};@
        );

        if function.inputs(db).iter().any(|type_| {
            matches!(
                type_,
                TransportTypeRs::JsValueHandle | TransportTypeRs::String
            )
        }) {
            frag!(
                {{
                    const handles = [];@
                    try {{
                        {inputs}
                        {call}
                    }} finally {{
                        for (const handle of handles) __rustier_values.__rustier_js_drop_ref(handle);@
                    }}
                }}
            )
            .print()
        } else {
            frag!({ { { call } } }).print()
        }
    }
}
