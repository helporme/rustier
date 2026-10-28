use crate::{
    codegen::fragment::{FragmentOnce, FragmentPrint, frag, fragment},
    db::Db,
    ir::transport::{TransportFunction, TransportTypeRs},
};

/// Native Node-API addon transport configuration.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TransportBackendNode;

pub(super) fn render_transport_preamble_js(
    _node: &TransportBackendNode,
    bin_file_path: &String,
) -> impl FragmentOnce {
    // Debug formatting escapes quotes, backslashes, and control characters.
    let path = format!("{bin_file_path:?}");
    frag!(
        import {{ createRequire as __rustier_create_require }} from "node:module";@
        const __rustier_exports = __rustier_create_require(import.meta.url)({path});@
    )
}

pub(super) fn render_transport_preamble_rs() -> impl FragmentOnce {
    frag!(
        use ::aux_next::transport::{{JsEnvHandle as __RustierEnvHandle, JsValueHandle as __RustierValueHandle, napi_sys as __rustier_napi}};@
    )
}

pub(super) fn render_registration<'db>(
    db: &'db dyn Db,
    functions: impl IntoIterator<Item = TransportFunction<'db>>,
) -> String {
    let exports = fragment(|p| {
        for function in functions {
            let name = function.symbol(db);
            let name_literal = format!("c{name:?}");
            let name_literal = name_literal.as_str();
            let length = name.len();
            frag!(
                let mut function = ::std::ptr::null_mut();@
                assert_eq!(unsafe {{
                    __rustier_napi::napi_create_function(env, {name_literal}.as_ptr(), {length}, Some({name}), ::std::ptr::null_mut(), &mut function)
                }}, __rustier_napi::Status::napi_ok);@
                assert_eq!(unsafe {{
                    __rustier_napi::napi_set_named_property(env, exports, {name_literal}.as_ptr(), function)
                }}, __rustier_napi::Status::napi_ok);@
            ).apply(p);
        }
    });
    frag!(
        #[unsafe(no_mangle)]@
        pub unsafe extern "C" fn napi_register_module_v1(
            env: __rustier_napi::napi_env,
            exports: __rustier_napi::napi_value
        ) -> __rustier_napi::napi_value {{
            {exports}
            exports
        }}
    )
    .print()
}

impl TransportBackendNode {
    pub(super) fn render_rs_adapter_with_body<'db>(
        &self,
        function: TransportFunction<'db>,
        db: &'db dyn Db,
        render_body: impl FnOnce(&str, &[String]) -> String,
    ) -> String {
        let name = function.symbol(db);
        let count = function.inputs(db).len();
        let arguments: Vec<_> = (0..count)
            .map(|index| format!("__rustier_handle_{index}"))
            .collect();
        let body = render_body("__rustier_env_handle", &arguments);
        let conversions = fragment(|p| {
            for (index, type_) in function.inputs(db).iter().enumerate() {
                match type_ {
                    TransportTypeRs::String => frag!(
                        let __rustier_handle_{index} = unsafe {{
                            __RustierValueHandle::from_napi_value(__rustier_args[{index}] as usize, __rustier_env_handle)
                        }}.to_string().expect("Expected a JavaScript string");@
                    ).apply(p),
                    TransportTypeRs::Bool => frag!(
                        let mut __rustier_handle_{index}: bool = false;@
                        let status = unsafe {{
                            __rustier_napi::napi_get_value_bool(env, __rustier_args[{index}], &mut __rustier_handle_{index})
                        }};@
                        if status != __rustier_napi::Status::napi_ok {{
                            panic!("Expected a JavaScript boolean");@
                        }}
                    ).apply(p),
                    TransportTypeRs::Void => frag!(
                        let __rustier_handle_{index} = ();@
                    ).apply(p),
                    TransportTypeRs::F64 => frag!(
                        let mut __rustier_handle_{index}: f64 = 0.0;@
                        let status = unsafe {{
                            __rustier_napi::napi_get_value_double(env, __rustier_args[{index}], &mut __rustier_handle_{index})
                        }};@
                        if status != __rustier_napi::Status::napi_ok {{
                            panic!("Expected a JavaScript number");@
                        }}
                    ).apply(p),
                    TransportTypeRs::JsValueHandle => frag!(
                        let __rustier_handle_{index} = unsafe {{
                            __RustierValueHandle::from_napi_value(__rustier_args[{index}] as usize, __rustier_env_handle)
                        }};@
                    ).apply(p),
                }
            }
        });
        let output = fragment(|p| match function.ret_type(db) {
            TransportTypeRs::String => frag!(unsafe {
                {
                    __rustier_env_handle
                        .new_string(&__rustier_output)
                        .into_napi_value() as __rustier_napi::napi_value
                }
            })
            .apply(p),
            TransportTypeRs::Bool => frag!(
                {{
                    let mut value = ::std::ptr::null_mut();@
                    let status = unsafe {{
                        __rustier_napi::napi_get_boolean(env, __rustier_output, &mut value)
                    }};@@
                    if status != __rustier_napi::Status::napi_ok {{
                        panic!("Could not create a JavaScript boolean");@
                    }}@
                    value
                }}
            )
            .apply(p),
            TransportTypeRs::Void => frag!(undefined).apply(p),
            TransportTypeRs::F64 => frag!(
                {{
                    let mut value = ::std::ptr::null_mut();@
                    let status = unsafe {{
                        __rustier_napi::napi_create_double(env, __rustier_output, &mut value)
                    }};@@
                    if status != __rustier_napi::Status::napi_ok {{
                        panic!("Could not create a JavaScript number");@
                    }}@
                    value
                }}
            )
            .apply(p),
            TransportTypeRs::JsValueHandle => frag!(unsafe {
                { __rustier_output.into_napi_value() as __rustier_napi::napi_value }
            })
            .apply(p),
        });
        frag!(
            unsafe extern "C" fn ~{name}(
                env: __rustier_napi::napi_env,
                info: __rustier_napi::napi_callback_info
            ) -> __rustier_napi::napi_value {{
                let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {{
                    let mut __rustier_args = [::std::ptr::null_mut(); {count}];@
                    let mut argc = {count};@
                    assert_eq!(unsafe {{
                        __rustier_napi::napi_get_cb_info(env, info, &mut argc, __rustier_args.as_mut_ptr(), ::std::ptr::null_mut(), ::std::ptr::null_mut())
                    }}, __rustier_napi::Status::napi_ok);@
                    let mut undefined = ::std::ptr::null_mut();@
                    assert_eq!(unsafe {{ __rustier_napi::napi_get_undefined(env, &mut undefined) }}, __rustier_napi::Status::napi_ok);@
                    for argument in __rustier_args.iter_mut().skip(argc) {{
                        *argument = undefined;@
                    }}
                    let __rustier_env_handle = unsafe {{ __RustierEnvHandle::from_raw(env as usize) }};@
                    {conversions}
                    let __rustier_output = {"{\n"}{body}{"\n};"}@
                    {output}
                }}));@
                match result {{
                    Ok(value) => value,
                    Err(_) => {{
                        unsafe {{ __rustier_napi::napi_throw_error(env, ::std::ptr::null(), c"Rust snippet panicked".as_ptr()); }}@
                        ::std::ptr::null_mut()
                    }}
                }}
            }}
        ).print()
    }

    pub(super) fn render_js_ffi_call_stmt<'db>(
        &self,
        function: TransportFunction<'db>,
        db: &'db dyn Db,
        arguments: &[String],
        result_var: &str,
    ) -> String {
        let name = function.symbol(db);
        let arguments = arguments.join(", ");
        frag!({result_var} = __rustier_exports.{name}{"("}{arguments}{");"}).print()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        codegen::TransportBackend,
        ir::js::{JsFile, JsProgram},
    };
    use std::{collections::BTreeMap, fs, process::Command};

    #[test]
    fn native_snippets_run_in_node() {
        let db = crate::test_common::database();
        let source = r#"
import { rs } from 'rustier';
export function echo(value) { return rs`${value}`; }
export function number() { return rs`__rustier_env.new_number(42.0)`; }
export function string() { return rs`__rustier_env.new_string("héllo\0🦀")`; }
export function clone(value) { return rs`${value}.clone()`; }
export function read(value) { return rs`let value = ${value}; __rustier_env.new_boolean(value.as_f64() == Some(42.0) && value.soft_eq(&__rustier_env.new_number(42.0)))`; }
export function text(value) { return rs`__rustier_env.new_string(${value}.to_string().unwrap())`; }
export function get(value, key) { return rs`${value}.get(${key}).unwrap_or_else(|error| error)`; }
export function call(value, arg) { return rs`${value}.apply(&[${arg}]).unwrap_or_else(|error| error)`; }
export function keys(value) { return rs`__rustier_env.new_number(${value}.own_keys().unwrap().len() as f64)`; }
export function panic() { return rs`panic!("test panic")`; }
"#;
        let file = JsFile::new(&db, "/input.mjs", source);
        let program = JsProgram::new(&db, vec![file]);
        assert!(program.diagnostics(&db).is_empty());
        let backend = TransportBackend::Node(TransportBackendNode);
        let directory = tempfile::tempdir().unwrap();
        let rust = ra_ap_hir::attach_db(&db, || program.runtime(&db).render_rs_ffi(&db, &backend));
        syn::parse_file(&rust).unwrap();
        fs::write(directory.path().join("lib.rs"), rust).unwrap();
        fs::write(
            directory.path().join("Cargo.toml"),
            program.render_cargo_toml(&db, &BTreeMap::new()),
        )
        .unwrap();
        let mut cargo = Command::new("cargo");
        cargo
            .args(["build", "--quiet", "--manifest-path"])
            .arg(directory.path().join("Cargo.toml"));
        if cfg!(target_os = "macos") {
            cargo.args(["--config", "build.rustflags=[\"-C\", \"link-arg=-undefined\", \"-C\", \"link-arg=dynamic_lookup\"]"]);
        }
        let output = cargo.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let library = format!(
            "{}rustier_macros{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        );
        fs::copy(
            directory.path().join("target/debug").join(library),
            directory.path().join("bridge.node"),
        )
        .unwrap();
        fs::write(
            directory.path().join("runtime.mjs"),
            ra_ap_hir::attach_db(&db, || {
                program
                    .runtime(&db)
                    .render_js_ffi(&db, &backend, &"./bridge.node".into())
            }),
        )
        .unwrap();
        fs::write(
            directory.path().join("app.mjs"),
            file.module(&db)
                .render_transformed(&db, "./runtime.mjs".into()),
        )
        .unwrap();
        // Transformation preserves imports; provide the tag module that a
        // bundler would normally tree-shake after replacing all tagged calls.
        let tag = directory.path().join("node_modules/rustier");
        fs::create_dir_all(&tag).unwrap();
        fs::write(
            tag.join("package.json"),
            r#"{"type":"module","exports":"./index.mjs"}"#,
        )
        .unwrap();
        fs::write(
            tag.join("index.mjs"),
            "export function rs() { throw new Error('uncompiled snippet'); }",
        )
        .unwrap();
        fs::write(directory.path().join("test.mjs"), r#"
import assert from 'node:assert/strict';
import * as app from './app.mjs';
for (const value of [undefined, null, true, false, 1, -0, NaN, 1n, 'héllo\0🦀', Symbol('key'), {}, () => {}]) {
    assert.ok(Object.is(app.echo(value), value));
    assert.ok(Object.is(app.clone(value), value));
}
assert.equal(app.number(), 42);
assert.equal(app.string(), 'héllo\0🦀');
assert.equal(app.read(42), true);
assert.equal(app.text('héllo\0🦀'), 'héllo\0🦀');
Object.defineProperty(Object.prototype, 'value', {configurable: true, set() {throw new Error('inherited setter')}});
try {
    assert.equal(app.echo(42), 42);
} finally {
    delete Object.prototype.value;
}
const symbol = Symbol('key');
assert.equal(app.keys({x: 1, [symbol]: 2}), 2);
assert.equal(app.get({[symbol]: 7}, symbol), 7);
const thrown = {error: true};
assert.equal(app.get({get x() {throw thrown}}, 'x'), thrown);
assert.equal(app.call(value => value + 1, 3), 4);
assert.equal(app.call(() => {throw thrown}, null), thrown);
assert.throws(() => app.panic(), /Rust snippet panicked/);
assert.equal(app.number(), 42);
"#).unwrap();
        let output = Command::new("node")
            .arg(directory.path().join("test.mjs"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
