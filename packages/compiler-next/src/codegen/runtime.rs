use crate::{
    codegen::{
        TransportBackend,
        fragment::{FragmentOnce, FragmentPrint, frag, fragment},
        transport::{
            render_transport_preamble_js, render_transport_preamble_rs,
            render_transport_registration_rs,
        },
    },
    db::Db,
    ir::runtime::{JsType, RsMacro, RsPart, RsType, Runtime},
};

impl<'db> Runtime<'db> {
    /// JavaScript runtime initialization and specialized macro exports.
    pub fn render_js_ffi(
        self,
        db: &'db dyn Db,
        backend: &TransportBackend,
        bin_file_path: &String,
    ) -> String {
        fragment(|printer| {
            render_transport_preamble_js(backend, bin_file_path).apply(printer);
            for macro_ in self.macros(db) {
                frag!(@{macro_.render_js_ffi(db, backend)}@).apply(printer);
            }
        })
        .print()
    }

    /// Rust FFI crate, including transport setup and macro exports.
    pub fn render_rs_ffi(self, db: &'db dyn Db, backend: &TransportBackend) -> String {
        fragment(|printer| {
            render_transport_preamble_rs(backend).apply(printer);
            for macro_ in self.macros(db) {
                frag!(@{macro_.render_rs_ffi(db, backend)}@).apply(printer);
            }
            render_transport_registration_rs(
                backend,
                db,
                self.macros(db).iter().map(|macro_| macro_.transport(db)),
            )
            .apply(printer);
        })
        .print()
    }
}

impl RsType {
    fn render(self) -> impl FragmentOnce {
        match self {
            Self::JsValue => frag!(::aux_next::runtime::js::JsValue),
        }
    }

    fn render_from_transport(self, value: impl FragmentOnce) -> impl FragmentOnce {
        match self {
            Self::JsValue => frag!(::aux_next::runtime::js::JsValue::from({ value })),
        }
    }

    fn render_to_transport(self, value: impl FragmentOnce) -> impl FragmentOnce {
        match self {
            Self::JsValue => frag!(::aux_next::transport::JsValueHandle::from({ value })),
        }
    }
}

impl<'db> RsMacro<'db> {
    fn render_rs_ffi(self, db: &'db dyn Db, backend: &TransportBackend) -> String {
        self.transport(db)
            .render_rs_adapter_with_body(db, backend, |env, arguments| {
                let ret_type = self.ret_type(db).render();
                let conversions = fragment(|printer| {
                    for (index, (type_, argument)) in
                        self.inputs(db).iter().zip(arguments).enumerate()
                    {
                        let value = type_.render_from_transport(argument);
                        frag!(let __rustier_arg_{index} = {value};@).apply(printer);
                    }
                });
                let body = fragment(|printer| {
                    let mut index = 0;
                    for part in self.body(db) {
                        match part {
                            RsPart::Text(text) => text.apply(printer),
                            RsPart::Input => {
                                frag!(__rustier_arg_ { index }).apply(printer);
                                index += 1;
                            }
                        }
                    }
                });
                let output = self
                    .ret_type(db)
                    .render_to_transport(frag!(__rustier_output));

                frag!(
                    let __rustier_env = ::aux_next::runtime::js::JsEnv::from({env});@
                    {conversions}@
                    let __rustier_output = {"(|| -> "}{ret_type}{" {\n"}{body}{"\n})();"}@
                    {output}
                )
                .print()
            })
    }

    fn render_js_ffi(self, db: &'db dyn Db, backend: &TransportBackend) -> String {
        let transport = self.transport(db);
        let name = self.symbol(db);
        let arguments: Vec<_> = self
            .inputs(db)
            .iter()
            .enumerate()
            .map(|(index, _)| frag!(__rustier_arg_ { index }).print())
            .collect();
        let params = arguments.join(", ");
        let transport_arguments: Vec<_> = self
            .inputs(db)
            .iter()
            .zip(&arguments)
            .map(|(type_, argument)| match type_.js() {
                JsType::Any => argument.clone(),
            })
            .collect();
        let result_var = "__rustier_result";
        let call = transport.render_js_ffi_call_stmt(db, backend, &transport_arguments, result_var);
        let result = match self.ret_type(db).js() {
            JsType::Any => result_var,
        };

        frag!(
            export function ~{name}{"("}{params}{")"} {{
                let ~{result_var};@
                {call}@
                return ~{result};@
            }}
        )
        .print()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{TransportBackendWasm, WasmImport};
    use crate::db::RootDatabase;

    fn test_backend() -> TransportBackend {
        TransportBackend::Wasm(TransportBackendWasm {
            mem64: false,
            import: WasmImport::Fetch,
        })
    }

    fn snapshot(name: &str, output: String) {
        insta::with_settings!({
            snapshot_path => "_snapshots/runtime",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!(name, output);
        });
    }

    fn render(body: Vec<RsPart>) -> String {
        let db = RootDatabase::default();
        let runtime = RsMacro::new(&db, body);
        let source = runtime.render_rs_ffi(&db, &test_backend());
        let result = prettyplease::unparse(&syn::parse_file(&source).unwrap());
        let input = render_input(runtime.body(&db));
        format!("# Input\n\n```js\n{input}\n```\n\n# Result\n\n```rust\n{result}```\n")
    }

    fn render_input(body: &[RsPart]) -> String {
        let mut input = String::from("rs`");
        let mut index = 0;
        for part in body {
            match part {
                RsPart::Text(text) => input.push_str(text),
                RsPart::Input => {
                    input.push_str(&format!("${{input{index}}}"));
                    index += 1;
                }
            }
        }
        input.push('`');
        input
    }

    fn render_js(body: Vec<RsPart>) -> String {
        let db = RootDatabase::default();
        let runtime = RsMacro::new(&db, body);
        let result = runtime.render_js_ffi(&db, &test_backend());
        let input = render_input(runtime.body(&db));
        format!("# Input\n\n```js\n{input}\n```\n\n# Result\n\n```js\n{result}\n```\n")
    }

    #[test]
    fn runtime_interpolation() {
        snapshot("runtime_interpolation", render(vec![RsPart::Input]));
    }

    #[test]
    fn runtime_multiple_inputs() {
        snapshot(
            "runtime_multiple_inputs",
            render(vec![
                RsPart::Text("let left = ".into()),
                RsPart::Input,
                RsPart::Text("; let right = ".into()),
                RsPart::Input,
                RsPart::Text("; if left.strict_eq(&right) { left } else { right }".into()),
            ]),
        );
    }

    #[test]
    fn runtime_constant() {
        snapshot(
            "runtime_constant",
            render(vec![RsPart::Text("__rustier_env.new_number(10.0)".into())]),
        );
    }

    #[test]
    fn runtime_multiline_literal() {
        snapshot(
            "runtime_multiline_literal",
            render(vec![RsPart::Text(
                "__rustier_env.new_string(r#\"first\nsecond\"#)".into(),
            )]),
        );
    }

    #[test]
    fn runtime_js_value_type() {
        snapshot(
            "runtime_js_value_type",
            format!("```rust\n{}\n```\n", RsType::JsValue.render().print()),
        );
    }

    #[test]
    fn runtime_js_no_inputs() {
        snapshot(
            "runtime_js_no_inputs",
            render_js(vec![RsPart::Text("__rustier_env.null()".into())]),
        );
    }

    #[test]
    fn runtime_js_multiple_inputs() {
        snapshot(
            "runtime_js_multiple_inputs",
            render_js(vec![
                RsPart::Text("let first = ".into()),
                RsPart::Input,
                RsPart::Text("; let second = ".into()),
                RsPart::Input,
                RsPart::Text("; first".into()),
            ]),
        );
    }

    #[test]
    fn program_runtime_tracks_and_deduplicates_macros() {
        use crate::ir::js::{JsFile, JsProgram};
        use salsa::Setter;

        let mut db = RootDatabase::default();
        let first = JsFile::new(&db, "/first.js", "import { rs } from 'rustier'; rs`${a}`;");
        let second = JsFile::new(&db, "/second.js", "import { rs } from 'rustier'; rs`${b}`;");
        let program = JsProgram::new(&db, vec![first, second]);
        let runtime = program.runtime(&db);
        assert!(runtime == program.runtime(&db));
        assert_eq!(runtime.macros(&db).len(), 1);
        let backend = test_backend();
        let js = runtime.render_js_ffi(&db, &backend, &"./bridge.wasm".into());
        assert_eq!(js.matches("export function ").count(), 1);
        assert_eq!(js.matches("WebAssembly.instantiate(").count(), 1);
        assert!(js.contains("\"./bridge.wasm\""));
        assert!(!js.contains("export{"));

        let changed = JsFile::new(
            &db,
            "/second.js",
            "import { rs } from 'rustier'; rs`__rustier_env.null()`;",
        );
        program.set_files(&mut db).to(vec![first, changed]);
        assert_eq!(program.runtime(&db).macros(&db).len(), 2);
    }
}
