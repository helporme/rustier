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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::transport::TransportBackend;
    use crate::{
        db::RootDatabase,
        ir::js::{JsFile, JsProgram},
    };

    fn test_backend() -> TransportBackend {
        TransportBackend::Wasm(TransportBackendWasm {
            mem64: false,
            import: WasmImport::Fetch,
        })
    }

    fn snapshot(name: &str, source: &str, backend: &TransportBackend) {
        let db = crate::test_common::database();
        let file = JsFile::new(&db, "/input.ts", source);
        let program = JsProgram::new(&db, vec![file]);
        let runtime = program.runtime(&db);
        let rust = ra_ap_hir::attach_db(&db, || runtime.render_rs_ffi(&db, backend));
        let result = prettyplease::unparse(&syn::parse_file(&rust).unwrap());
        let mut output =
            format!("# Source\n\n```ts\n{source}\n```\n\n# Result\n\n```rust\n{result}```\n");
        output.push_str(&format!(
            "\n# JS runtime\n\n```js\n{}\n```\n",
            ra_ap_hir::attach_db(&db, || runtime.render_js_ffi(
                &db,
                backend,
                &"./rustier.wasm".into()
            )),
        ));
        insta::with_settings!({
            snapshot_path => "../_snapshots/transport",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!(name, output);
        });
    }

    #[test]
    fn numeric_transport() {
        let db = RootDatabase::default();

        for (name, backend) in [
            ("number_wasm32", test_backend()),
            (
                "number_wasm64",
                TransportBackend::Wasm(TransportBackendWasm {
                    mem64: true,
                    import: WasmImport::NodeJs,
                }),
            ),
            (
                "number_node",
                TransportBackend::Node(crate::codegen::TransportBackendNode),
            ),
        ] {
            let function = TransportFunction::new(
                &db,
                "numbers",
                vec![
                    TransportTypeRs::F64,
                    TransportTypeRs::JsValueHandle,
                    TransportTypeRs::Void,
                    TransportTypeRs::F64,
                    TransportTypeRs::JsValueHandle,
                ],
                TransportTypeRs::F64,
            );
            let rs = function.render_rs_adapter_with_body(&db, &backend, |_, args| {
                format!("{} + {}", args[0], args[3])
            });
            let rs = prettyplease::unparse(&syn::parse_file(&rs).unwrap());
            let js = function.render_js_ffi_call_stmt(
                &db,
                &backend,
                &[
                    "a".into(),
                    "b".into(),
                    "undefined".into(),
                    "c".into(),
                    "d".into(),
                ],
                "output",
            );

            insta::with_settings!({
                snapshot_path => "../_snapshots/transport",
                prepend_module_to_snapshot => false,
            }, {
                insta::assert_snapshot!(name, format!("```rust\n{rs}```\n\n```js\n{js}\n```\n"));
            });
        }
    }

    #[test]
    fn string_transport() {
        let db = RootDatabase::default();

        for (name, backend) in [
            ("strings_wasm32", test_backend()),
            (
                "strings_wasm64",
                TransportBackend::Wasm(TransportBackendWasm {
                    mem64: true,
                    import: WasmImport::NodeJs,
                }),
            ),
            (
                "strings_node",
                TransportBackend::Node(crate::codegen::TransportBackendNode),
            ),
        ] {
            let function = TransportFunction::new(
                &db,
                "text",
                vec![
                    TransportTypeRs::String,
                    TransportTypeRs::F64,
                    TransportTypeRs::JsValueHandle,
                    TransportTypeRs::Void,
                    TransportTypeRs::Bool,
                    TransportTypeRs::String,
                ],
                TransportTypeRs::String,
            );
            let rs = function.render_rs_adapter_with_body(&db, &backend, |_, args| {
                let first = &args[0];
                let last = &args[5];

                frag!({ first } + &{ last }).print()
            });
            let rs = prettyplease::unparse(&syn::parse_file(&rs).unwrap());
            let js = function.render_js_ffi_call_stmt(
                &db,
                &backend,
                &[
                    "first".into(),
                    "number".into(),
                    "value".into(),
                    "undefined".into(),
                    "flag".into(),
                    "last".into(),
                ],
                "output",
            );
            let output = frag!({"```rust\n"}{rs}{"```\n\n```js\n"}{js}{"\n```\n"}).print();

            insta::with_settings!({
                snapshot_path => "../_snapshots/transport",
                prepend_module_to_snapshot => false,
            }, {
                insta::assert_snapshot!(name, output);
            });
        }
    }

    #[test]
    fn bool_transport() {
        let db = RootDatabase::default();

        for (name, backend) in [
            ("bool_wasm32", test_backend()),
            (
                "bool_wasm64",
                TransportBackend::Wasm(TransportBackendWasm {
                    mem64: true,
                    import: WasmImport::NodeJs,
                }),
            ),
            (
                "bool_node",
                TransportBackend::Node(crate::codegen::TransportBackendNode),
            ),
        ] {
            let function = TransportFunction::new(
                &db,
                "choose",
                vec![
                    TransportTypeRs::Bool,
                    TransportTypeRs::JsValueHandle,
                    TransportTypeRs::Void,
                    TransportTypeRs::F64,
                    TransportTypeRs::Bool,
                ],
                TransportTypeRs::Bool,
            );
            let rs = function.render_rs_adapter_with_body(&db, &backend, |_, args| {
                let first = &args[0];
                let number = &args[3];
                let last = &args[4];

                frag!({ first } && { last } && { number } > 0.0).print()
            });
            let rs = prettyplease::unparse(&syn::parse_file(&rs).unwrap());
            let js = function.render_js_ffi_call_stmt(
                &db,
                &backend,
                &[
                    "first".into(),
                    "value".into(),
                    "undefined".into(),
                    "number".into(),
                    "last".into(),
                ],
                "output",
            );
            let output = frag!({"```rust\n"}{rs}{"```\n\n```js\n"}{js}{"\n```\n"}).print();

            insta::with_settings!({
                snapshot_path => "../_snapshots/transport",
                prepend_module_to_snapshot => false,
            }, {
                insta::assert_snapshot!(name, output);
            });
        }
    }

    #[test]
    fn interpolation() {
        snapshot(
            "interpolation",
            "import { rs } from 'rustier';\nrs`${value}`;",
            &test_backend(),
        );
    }

    #[test]
    fn multiple_inputs() {
        snapshot(
            "multiple_inputs",
            "import { rs } from 'rustier';\nrs`let a = ${left}; let b = ${right}; if a.strict_eq(&b) { a } else { b }`;",
            &test_backend(),
        );
    }

    #[test]
    fn constant() {
        for (mode, import) in [
            ("fetch", WasmImport::Fetch),
            ("node_js", WasmImport::NodeJs),
            ("module", WasmImport::Module),
        ] {
            let TransportBackend::Wasm(mut wasm) = test_backend() else {
                unreachable!()
            };
            wasm.import = import;
            let backend = TransportBackend::Wasm(wasm);
            snapshot(
                &format!("constant_{mode}"),
                "import { rs } from 'rustier';\nrs`__rustier_env.new_number(0.0)`;",
                &backend,
            );
        }
    }

    #[test]
    fn identical_macros() {
        snapshot(
            "identical_macros",
            "import { rs } from 'rustier';\nrs`${left}`;\nrs`${right}`;",
            &test_backend(),
        );
    }

    #[test]
    fn supplied_adapter_bodies() {
        let db = RootDatabase::default();
        let function = TransportFunction::new(
            &db,
            "echo",
            vec![TransportTypeRs::JsValueHandle],
            TransportTypeRs::JsValueHandle,
        );
        let rs = function.render_rs_adapter_with_body(&db, &test_backend(), |env, args| {
            format!("let _ = {env}; {}", args[0])
        });
        let rs = prettyplease::unparse(&syn::parse_file(&rs).unwrap());
        let js = function.render_js_ffi_call_stmt(&db, &test_backend(), &["arg0".into()], "output");
        let output = format!(
            "# Rust body\n\n```rust\nlet _ = env;\narg0\n```\n\n# Rust result\n\n```rust\n{rs}```\n\n# JS call\n\n```js\noutput = echo(arg0);\n```\n\n# JS result\n\n```js\n{js}\n```\n"
        );
        insta::with_settings!({
            snapshot_path => "../_snapshots/transport",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!("supplied_adapter_bodies", output);
        });
    }
}
