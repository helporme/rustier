//! Rust side of the ABI: exported wrapper functions.

use crate::compiler::{
    codegen::fragment::{FragmentOnce, Printer, comma_list, doubles, frag, fragment},
    db::Db,
    ir::transport::{
        TransportFunction, TransportReturn, TransportType, TransportUnion,
        backend::{TransportBackend, WasmMemory, WasmTransport},
    },
};

impl TransportBackend {
    fn render_header<'a>(self, name: impl FragmentOnce + 'a) -> impl FragmentOnce + 'a {
        let attr = match self {
            TransportBackend::Wasm(_) => "wasm_bindgen",
            TransportBackend::Napi => "napi",
        };

        frag!(
            #[{attr}(js_name = {doubles(name)})]
        )
    }

    fn render_param_type(self, type_: TransportType) -> impl FragmentOnce {
        fragment(move |p| match (type_, self) {
            (TransportType::Unit, TransportBackend::Wasm(_)) => "JsValue".apply(p),
            (TransportType::JsValueHandle, TransportBackend::Wasm(_)) => "u32".apply(p),
            (TransportType::JsValueHandle, TransportBackend::Napi) => "__JsValue".apply(p),
            (
                TransportType::usize,
                TransportBackend::Wasm(WasmTransport {
                    memory: WasmMemory::x32,
                }),
            ) => {
                "u32".apply(p);
            }
            (
                TransportType::usize,
                TransportBackend::Wasm(WasmTransport {
                    memory: WasmMemory::x64,
                }),
            ) => {
                "u64".apply(p);
            }
            (TransportType::usize, TransportBackend::Napi) => {
                "napi::bindgen_prelude::BigInt".apply(p)
            }
            (type_, _) => type_.render_rust().apply(p),
        })
    }

    fn render_return_type(self, type_: TransportType) -> impl FragmentOnce {
        fragment(move |p| match (type_, self) {
            (TransportType::JsValueHandle, TransportBackend::Wasm(_)) => "u32".apply(p),
            (TransportType::JsValueHandle, TransportBackend::Napi) => "__JsValue".apply(p),
            (
                TransportType::usize,
                TransportBackend::Wasm(WasmTransport {
                    memory: WasmMemory::x32,
                }),
            ) => {
                "u32".apply(p);
            }
            (TransportType::usize, _) => "u64".apply(p),
            (type_, _) => type_.render_rust().apply(p),
        })
    }

    fn render_js_value(self, value: impl FragmentOnce) -> impl FragmentOnce {
        fragment(move |p| match self {
            TransportBackend::Wasm(_) => frag!(({ value }).into_raw()).apply(p),
            TransportBackend::Napi => frag!(__JsValue({ value })).apply(p),
        })
    }
}

impl TransportType {
    /// The Rust type of a logical ABI value inside the wrapper body.
    pub fn render_rust(self) -> &'static str {
        match self {
            TransportType::Unit => "()",
            TransportType::f64 => "f64",
            TransportType::bool => "bool",
            TransportType::String => "String",
            TransportType::usize => "usize",
            TransportType::JsValueHandle => "rustier_ext::transport::JsValueHandle",
        }
    }
}

impl TransportReturn<'_> {
    pub fn render_rust(self) -> &'static str {
        match self {
            Self::Value(type_) => type_.render_rust(),
            Self::Union(_) => "__TransportReturn",
        }
    }
}

impl<'db> TransportUnion<'db> {
    pub fn render_rust_variant(self, index: usize, value: impl FragmentOnce) -> impl FragmentOnce {
        frag!(__TransportReturn::V { index }({ value }))
    }

    fn render_encode(
        self,
        db: &'db dyn Db,
        transport: TransportBackend,
    ) -> impl FragmentOnce + 'db {
        let branches = self
            .variants(db)
            .iter()
            .enumerate()
            .map(move |(index, type_)| {
                let binding = if *type_ == TransportType::Unit {
                    "()"
                } else {
                    "value"
                };
                let value = fragment(move |p| {
                    if *type_ == TransportType::JsValueHandle {
                        transport.render_js_value(binding).apply(p);
                    } else if *type_ == TransportType::usize {
                        let type_ = transport.render_return_type(*type_);

                        frag!(value as ~{type_}).apply(p);
                    } else {
                        binding.apply(p);
                    }
                });

                let encoded = fragment(move |p| match transport {
                    TransportBackend::Wasm(_) => {
                        let payload = fragment(move |p| {
                            if *type_ == TransportType::Unit {
                                "JsValue::UNDEFINED".apply(p);
                            } else {
                                frag!(JsValue::from({ value })).apply(p);
                            }
                        });

                        frag!(vec![JsValue::from({index}u32), {payload}]).apply(p);
                    }
                    TransportBackend::Napi => {
                        let variant = match type_ {
                            TransportType::Unit => "A",
                            TransportType::f64 => "B",
                            TransportType::bool => "C",
                            TransportType::String => "D",
                            TransportType::usize => "E",
                            TransportType::JsValueHandle => "F",
                        };
                        let either = if self.variants(db).contains(&TransportType::JsValueHandle) {
                            "Either6"
                        } else {
                            "Either5"
                        };

                        frag!(({index}u32, napi::bindgen_prelude::{either}::{variant}({value})))
                            .apply(p);
                    }
                });

                frag!(__TransportReturn::V{index}({binding}) => {encoded})
            });

        frag!(
            match result {{
                {comma_list(branches)}
            }}
        )
    }
}

#[derive(Clone, Copy)]
pub struct TransportFunctionRenderOps {
    pub transport: TransportBackend,
    pub inject_js_env: bool,
}

impl<'db> TransportFunction<'db> {
    pub fn render_rust_with_body<Body>(
        self,
        db: &'db dyn Db,
        ops: TransportFunctionRenderOps,
        body: impl FnOnce(TransportInput<'db>) -> Body + 'db,
    ) -> impl FragmentOnce + 'db
    where
        Body: FragmentOnce + 'db,
    {
        let params = self.params(db);

        FunctionFragment {
            db,
            symbol: self.symbol(db),
            params,
            ret_type: self.ret_type(db),
            ops,
            body: body(TransportInput { params }),
        }
    }
}

struct FunctionFragment<'db, Body> {
    db: &'db dyn Db,
    symbol: &'db str,
    params: &'db [TransportType],
    ret_type: TransportReturn<'db>,
    ops: TransportFunctionRenderOps,
    body: Body,
}

impl<'db, Body: FragmentOnce + 'db> FragmentOnce for FunctionFragment<'db, Body> {
    fn apply(self, p: &mut Printer) {
        let Self {
            db,
            symbol,
            params,
            ret_type,
            ops,
            body,
        } = self;
        let TransportFunctionRenderOps {
            transport,
            inject_js_env,
        } = ops;

        let boundary_type = fragment(move |p| match (ret_type, transport) {
            (TransportReturn::Value(type_), _) => transport.render_return_type(type_).apply(p),
            (TransportReturn::Union(_), TransportBackend::Wasm(_)) => "Vec<JsValue>".apply(p),
            (TransportReturn::Union(union), TransportBackend::Napi) => {
                if union.variants(db).contains(&TransportType::JsValueHandle) {
                    "(u32, napi::bindgen_prelude::Either6<(), f64, bool, String, u64, __JsValue>)"
                        .apply(p);
                } else {
                    "(u32, napi::bindgen_prelude::Either5<(), f64, bool, String, u64>)".apply(p);
                }
            }
        });

        let definition = fragment(move |p| {
            if let TransportReturn::Union(union) = ret_type {
                let variants = comma_list(
                    union
                        .variants(db)
                        .iter()
                        .enumerate()
                        .map(|(index, type_)| frag!(V { index }({ type_.render_rust() }))),
                );

                frag!(enum __TransportReturn {{ {variants} }}@@).apply(p);
            }
        });

        let declarations = fragment(move |p| {
            if inject_js_env && matches!(transport, TransportBackend::Napi) {
                frag!(__env: napi::Env).apply(p);

                if !params.is_empty() {
                    frag!(,~).apply(p);
                }
            }

            comma_list(
                params
                    .iter()
                    .enumerate()
                    .map(|(i, type_)| frag!(_{i}: ~{transport.render_param_type(*type_)})),
            )
            .apply(p);
        });

        let decode = fragment(move |p| {
            if inject_js_env {
                let raw = fragment(move |p| match transport {
                    TransportBackend::Wasm(_) => frag!(0).apply(p),
                    TransportBackend::Napi => frag!(__env.raw() as usize).apply(p),
                });

                frag!(
                    let __js_env = unsafe {{ rustier_ext::transport::JsEnvHandle::from_raw({raw}) }};@@
                )
                .apply(p);
            }

            for (i, type_) in params.iter().enumerate() {
                if *type_ == TransportType::JsValueHandle {
                    let value = fragment(move |p| match transport {
                        TransportBackend::Wasm(_) => frag!(
                            unsafe {{
                                rustier_ext::transport::JsValueHandle::clone_from_raw(_{i})
                            }}
                        )
                        .apply(p),
                        TransportBackend::Napi => frag!(_{i}.0).apply(p),
                    });

                    frag!(let _{i}: rustier_ext::transport::JsValueHandle = {value};@@).apply(p);
                    continue;
                }

                if *type_ == TransportType::Unit && matches!(transport, TransportBackend::Wasm(_)) {
                    frag!(let _{i} = ();@@).apply(p);
                    continue;
                }

                if *type_ != TransportType::usize {
                    continue;
                }

                let value = fragment(move |p| match transport {
                    TransportBackend::Wasm(_) => frag!(_{i}).apply(p),
                    TransportBackend::Napi => frag!(_{i}.get_u64().1).apply(p),
                });

                frag!(
                    let _{i} = {value}~ as usize;@@
                )
                .apply(p);
            }
        });

        let encode = fragment(move |p| match ret_type {
            TransportReturn::Value(TransportType::JsValueHandle) => {
                transport.render_js_value("result").apply(p)
            }
            TransportReturn::Value(TransportType::usize) => {
                let type_ = transport.render_return_type(TransportType::usize);

                frag!(result as ~{type_}).apply(p);
            }
            TransportReturn::Value(_) => "result".apply(p),
            TransportReturn::Union(union) => union.render_encode(db, transport).apply(p),
        });

        frag!(
            {transport.render_header(symbol)}@
            pub fn ~{symbol}({declarations})~ -> ~{boundary_type}~ {{
                {decode}
                {definition}
                let result: {ret_type.render_rust()} = {{
                    {body}
                }};@@

                {encode}
            }}
        )
        .apply(p);
    }
}

/// The logical input of an exported wrapper function.
#[derive(Clone, Copy)]
pub struct TransportInput<'db> {
    params: &'db [TransportType],
}

impl<'db> TransportInput<'db> {
    pub fn env_expr(self) -> impl FragmentOnce + Copy {
        frag!(__js_env)
    }

    pub fn get(self, idx: usize) -> TransportArg {
        TransportArg {
            idx,
            type_: self.params[idx],
        }
    }
}

/// A logical argument of an exported wrapper function.
#[derive(Clone, Copy)]
pub struct TransportArg {
    idx: usize,
    type_: TransportType,
}

impl TransportArg {
    /// The logical ABI type of this argument.
    pub fn type_(self) -> TransportType {
        self.type_
    }

    /// An expression evaluating to the value of this argument.
    pub fn expr(self) -> impl FragmentOnce + Copy {
        let idx = self.idx;

        frag!(_{idx})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::{codegen::fragment::FragmentPrint, db::RootDatabase};

    fn snapshot(name: &str, render: impl Fn(&dyn Db, TransportBackend) -> String) {
        let db = RootDatabase::default();
        let snapshot = fragment(|p| {
            for (label, transport) in [
                ("N-API", TransportBackend::Napi),
                (
                    "WASM32",
                    TransportBackend::Wasm(WasmTransport {
                        memory: WasmMemory::x32,
                    }),
                ),
                (
                    "WASM64",
                    TransportBackend::Wasm(WasmTransport {
                        memory: WasmMemory::x64,
                    }),
                ),
            ] {
                let compiled = render(&db, transport);
                let syntax = syn::parse_file(&compiled)
                    .unwrap_or_else(|error| panic!("invalid generated Rust: {error}\n{compiled}"));
                let formatted = prettyplease::unparse(&syntax);

                ("## ", label, "\n\n```rust\n", formatted, "```\n\n").apply(p);
            }
        })
        .print();

        insta::with_settings!({
            snapshot_path => "_snapshots/rust",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!(name, snapshot);
        });
    }

    #[test]
    fn value_returns() {
        snapshot("value_returns", |db, transport| {
            fragment(|p| {
                for (symbol, type_) in [
                    ("unit", TransportType::Unit),
                    ("number", TransportType::f64),
                    ("boolean", TransportType::bool),
                    ("string", TransportType::String),
                    ("handle", TransportType::usize),
                ] {
                    let params = [type_];

                    FunctionFragment {
                        db,
                        symbol,
                        params: &params,
                        ret_type: TransportReturn::Value(type_),
                        ops: TransportFunctionRenderOps {
                            transport,
                            inject_js_env: false,
                        },
                        body: "_0",
                    }
                    .apply(p);
                    frag!(@@).apply(p);
                }
            })
            .print()
        });
    }

    #[test]
    fn js_env_injection() {
        snapshot("js_env_injection", |db, transport| {
            let only_env = FunctionFragment {
                db,
                symbol: "only_env",
                params: &[],
                ret_type: TransportReturn::Value(TransportType::Unit),
                ops: TransportFunctionRenderOps {
                    transport,
                    inject_js_env: true,
                },
                body: frag!(source::only_env(__js_env.into())),
            };
            let mixed_args = FunctionFragment {
                db,
                symbol: "mixed_args",
                params: &[TransportType::f64, TransportType::bool],
                ret_type: TransportReturn::Value(TransportType::f64),
                ops: TransportFunctionRenderOps {
                    transport,
                    inject_js_env: true,
                },
                body: frag!(source::mixed_args(_0, __js_env.into(), _1)),
            };

            frag!({only_env}@@{mixed_args}).print()
        });
    }

    #[test]
    fn union_returns() {
        snapshot("union_returns", |db, transport| {
            let union = TransportUnion::new(
                db,
                vec![
                    TransportType::Unit,
                    TransportType::f64,
                    TransportType::bool,
                    TransportType::String,
                    TransportType::usize,
                    TransportType::String,
                    TransportType::Unit,
                ],
            );
            let ret_type = TransportReturn::Union(union);
            let values = [
                "()",
                "42.5",
                "true",
                r#"String::from("first")"#,
                "7usize",
                "String::new()",
                "()",
            ];
            let branches = comma_list(values.into_iter().enumerate().map(|(index, value)| {
                let value = union.render_rust_variant(index, value);

                frag!({index} => {value})
            }));
            let fallback = union.render_rust_variant(0, "()");
            let body = frag!(
                match _0 as u32 {{
                    {branches},@
                    _ => {fallback},
                }}
            );

            FunctionFragment {
                db,
                symbol: "choose",
                params: &[TransportType::f64],
                ret_type,
                ops: TransportFunctionRenderOps {
                    transport,
                    inject_js_env: false,
                },
                body,
            }
            .print()
        });
    }
}
