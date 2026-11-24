use crate::{
    codegen::{
        TransportBackend,
        fragment::{FragmentOnce, FragmentPrint, comma_list, doubles, frag, fragment},
        transport::{
            render_transport_preamble_js, render_transport_preamble_rs,
            render_transport_registration_rs,
        },
    },
    db::Db,
    ir::runtime::{BigIntType, HandleBorrow, JsMacro, JsType, RsMacro, RsPart, RsType, Runtime},
    ir::transport::{TransportFunction, TransportTypeRs},
};

mod imports;

pub fn runtime_exports<'db>(db: &'db dyn Db, runtime: Runtime<'db>) -> Vec<String> {
    runtime
        .rs_macros(db)
        .iter()
        .filter(|macro_| macro_.is_compilable(db))
        .map(|macro_| macro_.symbol(db).clone())
        .collect()
}

impl<'db> Runtime<'db> {
    /// JavaScript runtime initialization and specialized macro exports.
    pub fn render_js_ffi(
        self,
        db: &'db dyn Db,
        backend: &TransportBackend,
        bin_file_path: &String,
    ) -> String {
        fragment(|p| {
            let imports = self.js_imports(db);
            render_transport_preamble_js(backend, bin_file_path, &imports).apply(p);
            if self.uses_rust_handles(db) {
                self.render_runtime_handles_preamble_js(db, backend)
                    .apply(p);
            } else {
                frag!(const __rustier_has_handle = () => false;@).apply(p);
            }
            self.render_js_imports(db, backend, &imports).apply(p);
            for macro_ in self
                .rs_macros(db)
                .iter()
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
            let imports = self.js_imports(db);
            let registration = self.render_rs_imports(db, backend, &imports, p);
            if self
                .js_macros(db)
                .iter()
                .any(|mac| mac.input_count(db) != 0)
            {
                self.render_js_input_conversion(db).apply(p);
            }
            for mac in self.js_macros(db) {
                mac.render_rs_boundary(db).apply(p);
            }
            for macro_ in self
                .rs_macros(db)
                .iter()
                .filter(|macro_| macro_.is_compilable(db))
            {
                frag!(@{macro_.render_rs_ffi(db, backend)}@).apply(p);
            }
            render_transport_registration_rs(
                backend,
                db,
                registration,
                self.rs_macros(db)
                    .iter()
                    .filter(|macro_| macro_.is_compilable(db))
                    .map(|mac| TransportFunction::lower(db, mac.symbol(db), mac.signature(db)))
                    .chain(
                        self.uses_rust_handles(db)
                            .then(|| TransportFunction::handle_drop(db)),
                    ),
            )
            .apply(p);
        })
        .print()
    }

    fn render_js_input_conversion(self, db: &'db dyn Db) -> impl FragmentOnce {
        let opaque = fragment(move |p| {
            if self.has_js_handle_inputs(db) {
                frag!(
                    Err(handle) => {{
                        let pointer = env_handle.new_string(&handle.as_raw().to_string());@
                        let value = import___rustier_new_handle(env, [pointer.into()])?;@
                        handle.into_raw();@
                        Ok(value)
                    }}
                )
                .apply(p);
            } else {
                frag!(Err(_) => unreachable!("no opaque inputs in this bridge")).apply(p);
            }
        });
        frag!(
            fn __rustier_input(env: ::rustier_runtime::JsEnv, input: ::rustier_runtime::js_call::Input)
            -> Result<::rustier_runtime::JsValue, ::rustier_runtime::JsCallError> {{
                let env_handle = __RustierEnvHandle::from(env);@
                match input.into_value(env_handle) {{
                    Ok(value) => Ok(value),@
                    {opaque}
                }}
            }}@
        )
    }

    fn render_runtime_handles_preamble_js(
        self,
        db: &'db dyn Db,
        backend: &TransportBackend,
    ) -> impl FragmentOnce {
        let call = TransportFunction::handle_drop(db).render_js_ffi_call_stmt(
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
        TransportFunction::handle_drop(db).render_rs_adapter_with_body(
            db,
            backend,
            |_, arguments| {
                let pointer = &arguments[0];

                frag!(unsafe {
                    {
                        ::rustier_runtime::rust_handles::RustHandle::drop_raw({ pointer });
                    }
                })
                .print()
            },
        )
    }
}

impl<'db> JsMacro<'db> {
    fn render_js_callback(self, db: &'db dyn Db) -> impl FragmentOnce {
        let params = comma_list((0..self.input_count(db)).map(|i| frag!(__rustier_input_ { i })));
        frag!(function ~{self.symbol(db)}({params}) {{ {self.body(db)} }}@)
    }

    fn render_rs_boundary(self, db: &'db dyn Db) -> impl FragmentOnce {
        let params =
            (0..self.input_count(db)).map(|i| frag!(, arg_{i}: ::rustier_runtime::js_call::Input));
        let params = fragment(move |p| {
            for param in params {
                param.apply(p);
            }
        });
        let args =
            comma_list((0..self.input_count(db)).map(|i| frag!(__rustier_input(env, arg_ { i })?)));
        frag!(
            #[unsafe(no_mangle)]@
            pub extern "Rust" fn ~{self.symbol(db)}(env: ::rustier_runtime::JsEnv{params})
            -> Result<::rustier_runtime::JsValue, ::rustier_runtime::JsCallError> {{
                import_{self.symbol(db)}(env, [{args}])
            }}@
        )
    }
}

impl RsType {
    pub(super) fn render(self) -> impl FragmentOnce {
        match self {
            Self::String => frag!(::std::string::String),
            Self::Bool => frag!(bool),
            Self::Void => frag!(()),
            Self::JsValue => frag!(::rustier_runtime::JsValue),
            Self::I8 => frag!(i8),
            Self::I16 => frag!(i16),
            Self::I32 => frag!(i32),
            Self::U8 => frag!(u8),
            Self::U16 => frag!(u16),
            Self::U32 => frag!(u32),
            Self::BigInt(_) => frag!({ TransportTypeRs::from(self).rust_name() }),
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
            Self::JsValue => frag!(::rustier_runtime::JsValue::from({ value })).apply(p),
            Self::F64 | Self::BigInt(_) => value.apply(p),
            Self::RustHandle { .. } => unreachable!(),
            Self::I8 | Self::I16 | Self::I32 | Self::U8 | Self::U16 | Self::U32 | Self::F32 => {
                let target = self.render();
                frag!(({value}) as ~{target}).apply(p);
            }
        })
    }

    fn render_to_transport(&self, value: impl FragmentOnce) -> impl FragmentOnce {
        fragment(move |p| match self {
            Self::Void | Self::Bool | Self::String => value.apply(p),
            Self::JsValue => frag!(::rustier_runtime::transport::JsValueHandle::from({ value })).apply(p),
            Self::F64 | Self::BigInt(_) => value.apply(p),
            Self::RustHandle { .. } => {
                frag!(::rustier_runtime::rust_handles::RustHandle::new({ value }).into_raw())
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
        TransportFunction::lower(db, self.symbol(db), self.signature(db))
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
                                    ::rustier_runtime::rust_handles::RustHandle::clone_from_raw({argument})
                                }};@
                                let mut __rustier_guard_{index} = __rustier_owned_{index}.{method}::<_>()
                                    .map_err(|error| ::rustier_runtime::transport::TransportError::Dynamic(error.to_string()))?;@
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
                                    ::rustier_runtime::transport::TransportError::Dynamic(error.to_string())
                                }})
                        ).apply(p);
                    } else if self.can_return_error(db) {
                        frag!(Ok({output})).apply(p);
                    } else {
                        output.apply(p);
                    }
                });

                let body = frag!(
                    let __rustier_env = ::rustier_runtime::JsEnv::from({env});@
                    #[allow(unused_imports)]@
                    use ::rustier::{{FromJs, JsCallError, JsEnv, JsMacro, JsValue, JsValueType, RsValueHandle, ToJs}};@
                    #[allow(unused_imports)]@
                    use ::rustier_js_proc::__js_in_rs as js;@
                    {conversions}@
                    let __rustier_output = {"(|| -> "}{ret_type}{" {\n"}{body}{"\n})();"}@
                    {output}
                ).print();

                if self.inputs(db).iter().any(|ty| matches!(ty, RsType::RustHandle { .. })) {
                    frag!((|| -> ::std::result::Result<_, ::rustier_runtime::transport::TransportError> {{
                        {body}
                    }})()).print()
                } else {
                    body
                }
            })
    }

    fn render_js_ffi(self, db: &'db dyn Db, backend: &TransportBackend) -> String {
        let transport = TransportFunction::lower(db, self.symbol(db), self.signature(db));
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
                            if (!__rustier_has_handle({argument})) {{
                                throw new TypeError({message});@
                            }}@
                        )
                        .apply(p);
                        continue;
                    }
                    JsType::String => "string",
                    JsType::Boolean => "boolean",
                    JsType::Number => "number",
                    JsType::BigInt => "bigint",
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

                let (min, max): (i64, i64) = match type_ {
                    RsType::BigInt(kind) => {
                        let (signedness, bits) = match kind {
                            BigIntType::Usize => match backend {
                                TransportBackend::Wasm(wasm) if !wasm.mem64 => ("asUintN", 32),
                                _ => ("asUintN", 64),
                            },
                            BigIntType::I64 => ("asIntN", 64),
                            BigIntType::U64 => ("asUintN", 64),
                            BigIntType::I128 => ("asIntN", 128),
                            BigIntType::U128 => ("asUintN", 128),
                        };
                        let message = doubles(frag!(
                            {"Rustier input "}{index}{": bigint outside "}{type_.render()}{" range"}
                        ));
                        frag!(
                            if (BigInt.{signedness}({bits}, {argument}) !== {argument}) {{
                                throw new TypeError({message});@
                            }}@
                        )
                        .apply(p);
                        continue;
                    }
                    RsType::I8 => (i8::MIN.into(), i8::MAX.into()),
                    RsType::I16 => (i16::MIN.into(), i16::MAX.into()),
                    RsType::I32 => (i32::MIN.into(), i32::MAX.into()),
                    RsType::U8 => (0, u8::MAX.into()),
                    RsType::U16 => (0, u16::MAX.into()),
                    RsType::U32 => (0, u32::MAX.into()),
                    RsType::F32 => {
                        let max = f64::from(f32::MAX).to_string();
                        let message =
                            doubles(frag!({"Rustier input "}{index}{": number outside f32 range"}));
                        frag!(
                            if (Math.abs({argument}) > {max}) {{
                                throw new TypeError({message});@
                            }}@
                        )
                        .apply(p);
                        continue;
                    }
                    _ => continue,
                };
                let message = doubles(frag!(
                    {"Rustier input "}{index}{": expected integer in "}{type_.render()}{" range"}
                ));
                frag!(
                    if (!Number.isInteger({argument}) || {argument} < {min} || {argument} > {max}) {{
                        throw new TypeError({message});@
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
                JsType::RustHandle => frag!({argument}[" __UNSAFE_ptr"]).print(),
                JsType::Any
                | JsType::Number
                | JsType::BigInt
                | JsType::Boolean
                | JsType::String => argument.clone(),
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
            JsType::RustHandle => frag!(__rustier_new_handle({ result_var })).print(),
            JsType::Any | JsType::Number | JsType::BigInt | JsType::Boolean | JsType::String => {
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
        let (db, rs) = crate::test_common::database();
        let runtime = RsMacro::new(&db, rs, body);
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
        let (db, rs) = crate::test_common::database();
        let runtime = RsMacro::new(&db, rs, body);
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
                RsPart::Text("; let _ = left; right".into()),
            ]),
        );
    }

    #[test]
    fn runtime_constant() {
        snapshot(
            "runtime_constant",
            render(vec![RsPart::Text("10.0f64".into())]),
        );
    }

    #[test]
    fn runtime_multiline_literal() {
        snapshot(
            "runtime_multiline_literal",
            render(vec![RsPart::Text("let _ = r#\"first\nsecond\"#;".into())]),
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
            render_js(vec![RsPart::Text("()".into())]),
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

        let (mut db, rs) = crate::test_common::database();
        let first = JsFile::new(&db, "/first.js", "import { rs } from 'rustier'; rs`${a}`;");
        let second = JsFile::new(&db, "/second.js", "import { rs } from 'rustier'; rs`${b}`;");
        let program = JsProgram::new(&db, vec![first, second]);
        let runtime = Runtime::new(&db, program, rs);
        assert_eq!(runtime.rs_macros(&db).len(), 1);
        let backend = test_backend();
        let js = ra_ap_hir::attach_db(&db, || {
            runtime.render_js_ffi(&db, &backend, &"./bridge.wasm".into())
        });
        assert_eq!(js.matches("export function ").count(), 1);
        assert_eq!(js.matches("WebAssembly.instantiate(").count(), 1);
        assert!(js.contains("\"./bridge.wasm\""));
        assert!(!js.contains("export{"));

        let changed = JsFile::new(&db, "/second.js", "import { rs } from 'rustier'; rs`()`;");
        program.set_files(&mut db).to(vec![first, changed]);
        let runtime = Runtime::new(&db, program, rs);
        assert_eq!(runtime.rs_macros(&db).len(), 2);
    }
}
