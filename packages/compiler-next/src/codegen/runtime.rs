use crate::{
    codegen::{
        TransportBackend,
        fragment::{FragmentOnce, FragmentPrint, doubles, frag, fragment},
        transport::{
            render_transport_preamble_js, render_transport_preamble_rs,
            render_transport_registration_rs,
        },
    },
    db::Db,
    ir::runtime::{HandleBorrow, JsType, RsMacro, RsPart, RsType, Runtime},
};

impl<'db> Runtime<'db> {
    /// JavaScript runtime initialization and specialized macro exports.
    pub fn render_js_ffi(
        self,
        db: &'db dyn Db,
        backend: &TransportBackend,
        bin_file_path: &String,
    ) -> String {
        fragment(|p| {
            render_transport_preamble_js(backend, bin_file_path).apply(p);
            if self.uses_rust_handles(db) {
                self.render_runtime_handles_preamble_js(db, backend)
                    .apply(p);
            }
            for macro_ in self
                .macros(db)
                .into_iter()
                .filter(|macro_| macro_.is_compilable(db))
            {
                frag!(@{macro_.render_js_ffi(db, backend)}@).apply(p);
            }
        })
        .print()
    }

    /// Rust FFI crate, including transport setup and macro exports.
    pub fn render_rs_ffi(self, db: &'db dyn Db, backend: &TransportBackend) -> String {
        fragment(|p| {
            render_transport_preamble_rs(backend).apply(p);
            if self.uses_rust_handles(db) {
                self.render_runtime_handles_preamble_rs(db, backend)
                    .apply(p);
            }
            for macro_ in self
                .macros(db)
                .into_iter()
                .filter(|macro_| macro_.is_compilable(db))
            {
                frag!(@{macro_.render_rs_ffi(db, backend)}@).apply(p);
            }
            render_transport_registration_rs(
                backend,
                db,
                self.macros(db)
                    .into_iter()
                    .filter(|macro_| macro_.is_compilable(db))
                    .map(|macro_| macro_.transport(db))
                    .chain(
                        self.uses_rust_handles(db)
                            .then(|| self.handle_drop_transport(db)),
                    ),
            )
            .apply(p);
        })
        .print()
    }

    fn render_runtime_handles_preamble_js(
        self,
        db: &'db dyn Db,
        backend: &TransportBackend,
    ) -> impl FragmentOnce {
        let call = self.handle_drop_transport(db).render_js_ffi_call_stmt(
            db,
            backend,
            &["pointer".into()],
            "__rustier_drop_result",
        );

        frag!(
            function __rustier_drop_handle(pointer) {{
                let __rustier_drop_result;@
                {call}
            }}@
            {include_str!("runtime/handles.mjs")}@
        )
    }

    fn render_runtime_handles_preamble_rs(
        self,
        db: &'db dyn Db,
        backend: &TransportBackend,
    ) -> String {
        self.handle_drop_transport(db)
            .render_rs_adapter_with_body(db, backend, |_, arguments| {
                let pointer = &arguments[0];

                frag!(unsafe {
                    {
                        ::aux_next::runtime::rust_handles::RustHandle::drop_raw({ pointer });
                    }
                })
                .print()
            })
    }
}

impl RsType {
    pub(super) fn render(self) -> impl FragmentOnce {
        match self {
            Self::String => frag!(::std::string::String),
            Self::Bool => frag!(bool),
            Self::Void => frag!(()),
            Self::JsValue => frag!(::aux_next::runtime::js::JsValue),
            Self::I8 => frag!(i8),
            Self::I16 => frag!(i16),
            Self::I32 => frag!(i32),
            Self::U8 => frag!(u8),
            Self::U16 => frag!(u16),
            Self::U32 => frag!(u32),
            Self::F32 => frag!(f32),
            Self::F64 => frag!(f64),
            Self::RustHandle {
                borrow: HandleBorrow::Owned,
            } => frag!(_),
            Self::RustHandle {
                borrow: HandleBorrow::Shared,
            } => frag!(&_),
            Self::RustHandle {
                borrow: HandleBorrow::Mutable,
            } => frag!(&mut _),
        }
    }

    fn render_from_transport(&self, value: impl FragmentOnce) -> impl FragmentOnce {
        fragment(move |p| match self {
            Self::Void | Self::Bool | Self::String => value.apply(p),
            Self::JsValue => frag!(::aux_next::runtime::js::JsValue::from({ value })).apply(p),
            Self::F64 => value.apply(p),
            Self::RustHandle { .. } => unreachable!(),
            Self::F32 => frag!(({ value }).clamp(f32::MIN as f64, f32::MAX as f64) as f32).apply(p),
            Self::I8 | Self::I16 | Self::I32 | Self::U8 | Self::U16 | Self::U32 => {
                let target = self.render();
                frag!(({value}) as ~{target}).apply(p);
            }
        })
    }

    fn render_to_transport(&self, value: impl FragmentOnce) -> impl FragmentOnce {
        fragment(move |p| match self {
            Self::Void | Self::Bool | Self::String => value.apply(p),
            Self::JsValue => frag!(::aux_next::transport::JsValueHandle::from({ value })).apply(p),
            Self::F64 => value.apply(p),
            Self::RustHandle { .. } => {
                frag!(::aux_next::runtime::rust_handles::RustHandle::new({ value }).into_raw())
                    .apply(p);
            }
            Self::I8 | Self::I16 | Self::I32 | Self::U8 | Self::U16 | Self::U32 | Self::F32 => {
                frag!(({ value }) as f64).apply(p)
            }
        })
    }
}

impl<'db> RsMacro<'db> {
    pub(super) fn render_result_type(self, db: &'db dyn Db) -> impl FragmentOnce {
        let value = self.ret_type(db).render();
        let error = if self.has_inferred_error_type(db) {
            frag!(_)
        } else {
            frag!(::std::boxed::Box<dyn ::std::error::Error + Send + Sync + 'static>)
        };

        frag!(::std::result::Result<{value}, {error}>)
    }

    fn render_rs_ffi(self, db: &'db dyn Db, backend: &TransportBackend) -> String {
        self.transport(db)
            .render_rs_adapter_with_body(db, backend, |env, arguments| {
                let ret_type = fragment(|p| {
                    if self.returns_result(db) {
                        self.render_result_type(db).apply(p);
                    } else {
                        self.ret_type(db).render().apply(p);
                    }
                });
                let conversions = fragment(|p| {
                    for (index, (type_, argument)) in
                        self.inputs(db).iter().zip(arguments).enumerate()
                    {
                        if let RsType::RustHandle { borrow } = type_ {
                            let method = match borrow {
                                HandleBorrow::Owned => "take",
                                HandleBorrow::Shared => "borrow",
                                HandleBorrow::Mutable => "borrow_mut",
                            };
                            let value = match borrow {
                                HandleBorrow::Owned => frag!(__rustier_guard_{index}),
                                HandleBorrow::Shared => frag!(&*__rustier_guard_{index}),
                                HandleBorrow::Mutable => frag!(&mut *__rustier_guard_{index}),
                            };

                            frag!(
                                let __rustier_owned_{index} = unsafe {{
                                    ::aux_next::runtime::rust_handles::RustHandle::clone_from_raw({argument})
                                }};@
                                let mut __rustier_guard_{index} = __rustier_owned_{index}.{method}::<_>()
                                    .map_err(|error| ::aux_next::transport::TransportError::Dynamic(error.to_string()))?;@
                                let __rustier_arg_{index} = {value};@
                            ).apply(p);
                            continue;
                        }

                        let value = type_.render_from_transport(argument);
                        frag!(let __rustier_arg_{index} = {value};@).apply(p);
                    }
                });
                let body = fragment(|p| {
                    let mut index = 0;
                    for part in self.body(db) {
                        match part {
                            RsPart::Text(text) => text.apply(p),
                            RsPart::Input => {
                                frag!(__rustier_arg_ { index }).apply(p);
                                index += 1;
                            }
                        }
                    }
                });
                let ret = self.ret_type(db);
                let output = ret.render_to_transport(frag!(__rustier_output));
                let output = fragment(|p| {
                    if self.returns_result(db) {
                        frag!(
                            __rustier_output
                                .map(|__rustier_output| {output})
                                .map_err(|error| {{
                                    let error: ::std::boxed::Box<dyn ::std::error::Error + Send + Sync + 'static> = error.into();@
                                    ::aux_next::transport::TransportError::Dynamic(error.to_string())
                                }})
                        ).apply(p);
                    } else if self.can_return_error(db) {
                        frag!(Ok({output})).apply(p);
                    } else {
                        output.apply(p);
                    }
                });

                let body = frag!(
                    let __rustier_env = ::aux_next::runtime::js::JsEnv::from({env});@
                    {conversions}@
                    let __rustier_output = {"(|| -> "}{ret_type}{" {\n"}{body}{"\n})();"}@
                    {output}
                ).print();

                if self.inputs(db).iter().any(|ty| matches!(ty, RsType::RustHandle { .. })) {
                    frag!((|| -> ::std::result::Result<_, ::aux_next::transport::TransportError> {{
                        {body}
                    }})()).print()
                } else {
                    body
                }
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
        let checks = fragment(|p| {
            for (index, (type_, argument)) in self.inputs(db).iter().zip(&arguments).enumerate() {
                let expected = match type_.js() {
                    JsType::RustHandle => {
                        let message =
                            doubles(frag!({"Rustier input "}{index}{": expected Rust handle"}));

                        frag!(
                            if (!__rustier_is_handle({argument})) {{
                                throw new TypeError({message});@
                            }}@
                        )
                        .apply(p);
                        continue;
                    }
                    JsType::String => "string",
                    JsType::Boolean => "boolean",
                    JsType::Number => "number",
                    JsType::Undefined => "undefined",
                    JsType::Any => continue,
                };
                let expected_literal = doubles(expected);
                let message_literal =
                    doubles(frag!({"Rustier input "}{index}{": expected "}{expected}));

                frag!(
                    if (typeof ~{argument} ~!== ~{expected_literal}) {{
                        throw new TypeError({message_literal});@
                    }}@
                )
                .apply(p);
            }
        });
        let transport_arguments: Vec<_> = self
            .inputs(db)
            .iter()
            .zip(&arguments)
            .map(|(type_, argument)| match type_.js() {
                JsType::Undefined => "undefined".into(),
                JsType::RustHandle => frag!(__rustier_handle_pointer({ argument })).print(),
                JsType::Any | JsType::Number | JsType::Boolean | JsType::String => argument.clone(),
            })
            .collect();
        let result_var = "__rustier_result";
        let call = transport.render_js_ffi_call_stmt(db, backend, &transport_arguments, result_var);
        let call = fragment(|p| {
            if transport.can_return_error(db) {
                frag!(
                    try {{
                        {call}@
                    }} catch (error) {{
                        if (error instanceof Error && error.name === "RustError" && typeof Error.captureStackTrace === "function") {{
                            Error.captureStackTrace(error, {name});@
                        }}@
                        throw error;@
                    }}
                )
                .apply(p);
            } else {
                call.apply(p);
            }
        });
        let result = match self.ret_type(db).js() {
            JsType::Undefined => "undefined".to_owned(),
            JsType::RustHandle => frag!(new Rs({result_var}, __rustier_handle_key)).print(),
            JsType::Any | JsType::Number | JsType::Boolean | JsType::String => {
                result_var.to_owned()
            }
        };

        frag!(
            export function ~{name}{"("}{params}{")"} {{
                {checks}
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
        let db = crate::test_common::database();
        let runtime = RsMacro::new(&db, body);
        let source = ra_ap_hir::attach_db(&db, || runtime.render_rs_ffi(&db, &test_backend()));
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
        let db = crate::test_common::database();
        let runtime = RsMacro::new(&db, body);
        let result = ra_ap_hir::attach_db(&db, || runtime.render_js_ffi(&db, &test_backend()));
        let input = render_input(runtime.body(&db));
        format!("# Input\n\n```js\n{input}\n```\n\n# Result\n\n```js\n{result}\n```\n")
    }

    #[test]
    fn runtime_numbers() {
        for type_ in ["i8", "i16", "i32", "u8", "u16", "u32", "f32", "f64"] {
            let body = vec![
                RsPart::Text(format!("let value: {type_} = ")),
                RsPart::Input,
                RsPart::Text("; value".into()),
            ];

            snapshot(
                &format!("runtime_{type_}"),
                format!("{}\n{}", render(body.clone()), render_js(body)),
            );
        }
    }

    #[test]
    fn runtime_string() {
        let body = vec![
            RsPart::Text("let value: std::string::String = ".into()),
            RsPart::Input,
            RsPart::Text("; value".into()),
        ];

        snapshot("runtime_string", render(body.clone()));
        snapshot("runtime_js_string", render_js(body));
    }

    #[test]
    fn runtime_bool() {
        let body = vec![
            RsPart::Text("let value: bool = ".into()),
            RsPart::Input,
            RsPart::Text("; !value".into()),
        ];

        snapshot("runtime_bool", render(body.clone()));
        snapshot("runtime_bool_js", render_js(body));
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
    fn runtime_js_input_checks() {
        snapshot(
            "runtime_js_input_checks",
            render_js(vec![
                RsPart::Text("let _: () = ".into()),
                RsPart::Input,
                RsPart::Text("; let _: f64 = ".into()),
                RsPart::Input,
                RsPart::Text("; ".into()),
                RsPart::Input,
            ]),
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

        let mut db = crate::test_common::database();
        let first = JsFile::new(&db, "/first.js", "import { rs } from 'rustier'; rs`${a}`;");
        let second = JsFile::new(&db, "/second.js", "import { rs } from 'rustier'; rs`${b}`;");
        let program = JsProgram::new(&db, vec![first, second]);
        let runtime = program.runtime(&db);
        assert!(runtime == program.runtime(&db));
        assert_eq!(runtime.macros(&db).len(), 1);
        let backend = test_backend();
        let js = ra_ap_hir::attach_db(&db, || {
            runtime.render_js_ffi(&db, &backend, &"./bridge.wasm".into())
        });
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
