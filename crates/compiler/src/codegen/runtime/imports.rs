use super::*;
use crate::{codegen::fragment::Printer, ir::transport::TransportTypeRs};

impl<'db> Runtime<'db> {
    pub(super) fn js_imports(self, db: &'db dyn Db) -> Vec<&'db str> {
        let mut names: Vec<_> = self
            .js_macros(db)
            .iter()
            .map(|mac| mac.symbol(db).as_str())
            .collect();

        if self.has_js_handle_inputs(db) {
            names.push("__rustier_new_handle");
        }
        names
    }

    pub(super) fn render_js_imports(
        self,
        db: &'db dyn Db,
        backend: &TransportBackend,
        imports: &[&str],
    ) -> impl FragmentOnce {
        fragment(move |p| {
            if imports.is_empty() {
                return;
            }
            for mac in self.js_macros(db) {
                mac.render_js_callback(db).apply(p);
            }
            if matches!(backend, TransportBackend::Node(_)) {
                let exports = comma_list(
                    self.rs_macros(db)
                        .iter()
                        .filter(|site| site.is_compilable(db))
                        .map(|site| site.symbol(db)),
                )
                .print();
                frag!(
                    if (__rustier_exports.__rustier_facade) {{
                        ({{ {&exports} }} = __rustier_exports.__rustier_facade);@
                    }} else {{
                        __rustier_exports.__rustier_bind({comma_list(imports)});@
                        __rustier_exports.__rustier_facade = {{ {&exports} }};@
                    }}@
                )
                .apply(p);
            }
        })
    }

    pub(super) fn render_rs_imports(
        self,
        db: &'db dyn Db,
        backend: &TransportBackend,
        imports: &[&str],
        p: &mut Printer,
    ) -> Option<TransportFunction<'db>> {
        if imports.is_empty() {
            return None;
        }

        for name in imports {
            let body = match backend {
                TransportBackend::Node(_) => frag!(
                    let handle = __RustierEnvHandle::from(_env);@
                    let imports = unsafe {{ handle.static_data::<__RustierBridgeState>() }}
                        .and_then(|state| state.get()).ok_or_else(::rustier_runtime::JsCallError::unbound)?;@
                    let value = imports.{name}.call(args.map(__RustierValueHandle::from)).map_err(::rustier_runtime::JsValue::from)?;@
                    Ok(value.into())
                ).print(),
                TransportBackend::Wasm(_) => frag!(
                    #[link(wasm_import_module = "rustier:js")]@
                    unsafe extern "C" {{
                        fn call_{name}(args: *const u32, count: usize, failed: *mut u32) -> u32;@
                    }}@
                    let handles = args.map(__RustierValueHandle::from);@
                    let raw = handles.each_ref().map(__RustierValueHandle::as_raw);@
                    let mut failed = 0;@
                    let result = unsafe {{ __RustierValueHandle::from_raw(call_{name}(raw.as_ptr(), N, &mut failed)) }};@
                    if failed == 0 {{ Ok(result.into()) }} else {{ Err(::rustier_runtime::JsCallError::from(::rustier_runtime::JsValue::from(result))) }}
                ).print(),
            };
            frag!(
                fn import_{name}<const N: usize>(_env: ::rustier_runtime::JsEnv, args: [::rustier_runtime::JsValue; N])
                -> Result<::rustier_runtime::JsValue, ::rustier_runtime::JsCallError> {{ {body} }}@
            ).apply(p);
        }

        if matches!(backend, TransportBackend::Wasm(_)) {
            return None;
        }
        let fields = comma_list(
            imports
                .iter()
                .map(|name| frag!({name}: ::rustier_runtime::transport::NodeFunction)),
        );
        frag!(
            struct __RustierImports {{ {fields} }}@
            type __RustierBridgeState = ::std::cell::OnceCell<__RustierImports>;@
        )
        .apply(p);
        let bind = TransportFunction::new(
            db,
            "__rustier_bind",
            vec![TransportTypeRs::JsValueHandle; imports.len()],
            TransportTypeRs::Void,
            true,
        );
        bind.render_rs_adapter_with_body(db, backend, |env, args| {
            let fields = comma_list(imports.iter().zip(args).map(
                |(name, arg)| frag!({name}: ::rustier_runtime::transport::NodeFunction::bind({arg})?),
            ));
            frag!((|| -> Result<(), ::rustier_runtime::transport::TransportError> {{
                let state = unsafe {{ {env}.static_data::<__RustierBridgeState>() }}.ok_or(
                    ::rustier_runtime::transport::TransportError::Static("Missing Rustier static data")
                )?;@
                if state.get().is_none() {{
                    let imports = __RustierImports {{ {fields} }};@
                    let _ = state.set(imports);@
                }}@
                Ok(())
            }})())
            .print()
        })
        .apply(p);
        Some(bind)
    }
}
