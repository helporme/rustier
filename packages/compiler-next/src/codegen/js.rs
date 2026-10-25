use std::fmt::Write as _;

use crate::{
    codegen::{
        TransportBackend,
        fragment::{FragmentOnce, FragmentPrint, frag, fragment},
        transport::{render_transport_preamble_js, render_transport_preamble_rs},
    },
    db::Db,
    ir::js::JsProgram,
};

impl JsProgram {
    /// Snippet exports importing the shared backend instead of instantiating it.
    pub fn render_js_ffi_exports(
        self,
        db: &dyn Db,
        backend: &TransportBackend,
        opts: &super::TransportCodegenOpts,
    ) -> String {
        let module_id = js_string(&opts.ffi_module_id);
        fragment(|printer| {
            frag!(
                import {{ __rustier_exports, __rustier_values }} from ~{module_id};@
            )
            .apply(printer);
            for macro_ in self.macros(db) {
                frag!(@{macro_.render_js_ffi(db, backend)}@).apply(printer);
            }
        })
        .print()
    }

    /// Renders the JS FFI module exporting one function per canonical snippet.
    /// Transport supplies backend initialization; runtime supplies function behavior.
    pub fn render_js_ffi(
        self,
        db: &dyn Db,
        backend: &TransportBackend,
        opts: &super::TransportCodegenOpts,
    ) -> String {
        fragment(|printer| {
            render_transport_preamble_js(backend, opts).apply(printer);
            for macro_ in self.macros(db) {
                frag!(@{macro_.render_js_ffi(db, backend)}@).apply(printer);
            }
        })
        .print()
    }

    /// Renders the Rust FFI crate, including transport setup and snippet exports.
    pub fn render_rs_ffi(self, db: &dyn Db, backend: &TransportBackend) -> String {
        fragment(|printer| {
            render_transport_preamble_rs(backend).apply(printer);
            for macro_ in self.macros(db) {
                frag!(@{macro_.render_rs_ffi(db, backend)}@).apply(printer);
            }
        })
        .print()
    }
}

pub(super) fn js_string(value: &str) -> impl FragmentOnce {
    fragment(move |printer| {
        "\"".apply(printer);
        for character in value.chars() {
            match character {
                '"' => "\\\"".apply(printer),
                '\\' => "\\\\".apply(printer),
                '\n' => "\\n".apply(printer),
                '\r' => "\\r".apply(printer),
                '\t' => "\\t".apply(printer),
                character
                    if character <= '\u{1f}' || matches!(character, '\u{2028}' | '\u{2029}') =>
                {
                    write!(printer, "\\u{:04x}", character as u32).unwrap();
                }
                character => write!(printer, "{character}").unwrap(),
            }
        }
        "\"".apply(printer);
    })
}

#[cfg(test)]
mod tests {
    use super::js_string;
    use crate::codegen::fragment::FragmentPrint;
    use crate::{
        codegen::{TransportBackend, TransportBackendWasm, WasmImport},
        db::RootDatabase,
        ir::js::{JsFile, JsProgram},
    };

    fn test_backend(import: WasmImport) -> TransportBackend {
        TransportBackend::Wasm(TransportBackendWasm {
            mem64: false,
            import,
        })
    }

    #[test]
    fn string_literal_escaping() {
        assert_eq!(
            js_string("\01\"\\\n\r\t\u{1f}\u{2028}\u{2029}é😀").print(),
            r#""\u00001\"\\\n\r\t\u001f\u2028\u2029é😀""#
        );
    }

    #[test]
    fn snippet_exports() {
        let db = RootDatabase::default();
        let source = "import { rs } from 'rustier';\nconst zero = rs`0`;\nconst identity = value => rs`${value}`;";
        let file = JsFile::new(&db, "/input.js", source);
        let program = JsProgram::new(&db, vec![file]);
        for (mode, import) in [
            ("fetch", WasmImport::Fetch),
            ("node_js", WasmImport::NodeJs),
            ("module", WasmImport::Module),
        ] {
            let backend = test_backend(import);
            let opts = super::super::TransportCodegenOpts {
                ffi_module_id: "rustier-next:runtime".into(),
                ffi_bin_id: "./rustier.wasm".into(),
            };
            let wrappers = program.render_js_ffi_exports(&db, &backend, &opts);
            let initialization = backend.render_js_backend(&opts);
            assert!(!wrappers.contains("WebAssembly"));
            assert!(!initialization.contains("rustier-next:values"));
            let output = format!(
                "# Source\n\n```js\n{source}\n```\n\n# Wrappers\n\n```js\n{wrappers}\n```\n\n# Shared backend\n\n```js\n{initialization}\n```\n"
            );
            let name = format!("snippet_exports_{}", mode);
            insta::with_settings!({
                snapshot_path => "_snapshots/js",
                prepend_module_to_snapshot => false,
            }, {
                insta::assert_snapshot!(name, output);
            });
        }
    }
}
