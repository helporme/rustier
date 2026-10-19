//! Rust side of the ABI: exported wrapper functions.

use crate::compiler::{
    codegen::fragment::{FragmentOnce, Printer, comma_list, doubles, frag, fragment},
    db::Db,
    ir::abi::{
        AbiFunction, AbiReturn, AbiType, AbiUnion,
        transport::{AbiTransport, WasmMemory, WasmTransport},
    },
};

impl AbiTransport {
    fn render_header<'a>(self, name: impl FragmentOnce + 'a) -> impl FragmentOnce + 'a {
        let attr = match self {
            AbiTransport::Wasm(_) => "wasm_bindgen",
            AbiTransport::Napi => "napi",
        };

        frag!(
            #[{attr}(js_name = {doubles(name)})]
        )
    }

    fn render_param_type(self, type_: AbiType) -> impl FragmentOnce {
        fragment(move |p| match (type_, self) {
            (AbiType::Unit, AbiTransport::Wasm(_)) => "JsValue".apply(p),
            (
                AbiType::usize,
                AbiTransport::Wasm(WasmTransport {
                    memory: WasmMemory::x32,
                }),
            ) => {
                "u32".apply(p);
            }
            (
                AbiType::usize,
                AbiTransport::Wasm(WasmTransport {
                    memory: WasmMemory::x64,
                }),
            ) => {
                "u64".apply(p);
            }
            (AbiType::usize, AbiTransport::Napi) => "napi::bindgen_prelude::BigInt".apply(p),
            (type_, _) => type_.render_rust().apply(p),
        })
    }

    fn render_return_type(self, type_: AbiType) -> impl FragmentOnce {
        fragment(move |p| match (type_, self) {
            (
                AbiType::usize,
                AbiTransport::Wasm(WasmTransport {
                    memory: WasmMemory::x32,
                }),
            ) => {
                "u32".apply(p);
            }
            (AbiType::usize, _) => "u64".apply(p),
            (type_, _) => type_.render_rust().apply(p),
        })
    }
}

impl AbiType {
    /// The Rust type of a logical ABI value inside the wrapper body.
    pub fn render_rust(self) -> &'static str {
        match self {
            AbiType::Unit => "()",
            AbiType::f64 => "f64",
            AbiType::bool => "bool",
            AbiType::String => "String",
            AbiType::usize => "usize",
        }
    }
}

impl AbiReturn<'_> {
    pub fn render_rust(self) -> &'static str {
        match self {
            Self::Value(type_) => type_.render_rust(),
            Self::Union(_) => "__AbiReturn",
        }
    }
}

impl<'db> AbiUnion<'db> {
    pub fn render_rust_variant(self, index: usize, value: impl FragmentOnce) -> impl FragmentOnce {
        frag!(__AbiReturn::V { index }({ value }))
    }

    fn render_encode(self, db: &'db dyn Db, transport: AbiTransport) -> impl FragmentOnce + 'db {
        let branches = self
            .variants(db)
            .iter()
            .enumerate()
            .map(move |(index, type_)| {
                let binding = if *type_ == AbiType::Unit {
                    "()"
                } else {
                    "value"
                };
                let value = fragment(move |p| {
                    if *type_ == AbiType::usize {
                        let type_ = transport.render_return_type(*type_);

                        frag!(value as ~{type_}).apply(p);
                    } else {
                        binding.apply(p);
                    }
                });

                let encoded = fragment(move |p| match transport {
                    AbiTransport::Wasm(_) => {
                        let payload = fragment(move |p| {
                            if *type_ == AbiType::Unit {
                                "JsValue::UNDEFINED".apply(p);
                            } else {
                                frag!(JsValue::from({ value })).apply(p);
                            }
                        });

                        frag!(vec![JsValue::from({index}u32), {payload}]).apply(p);
                    }
                    AbiTransport::Napi => {
                        let variant = match type_ {
                            AbiType::Unit => "A",
                            AbiType::f64 => "B",
                            AbiType::bool => "C",
                            AbiType::String => "D",
                            AbiType::usize => "E",
                        };

                        frag!(({index}u32, napi::bindgen_prelude::Either5::{variant}({value})))
                            .apply(p);
                    }
                });

                frag!(__AbiReturn::V{index}({binding}) => {encoded})
            });

        frag!(
            match result {{
                {comma_list(branches)}
            }}
        )
    }
}

#[derive(Clone, Copy)]
pub struct AbiFunctionRenderOps {
    pub transport: AbiTransport,
    pub inject_js_env: bool,
}

impl<'db> AbiFunction<'db> {
    pub fn render_rust_with_body<Body>(
        self,
        db: &'db dyn Db,
        ops: AbiFunctionRenderOps,
        body: impl FnOnce(AbiArgs<'db>) -> Body + 'db,
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
            body: body(AbiArgs { params }),
        }
    }
}

struct FunctionFragment<'db, Body> {
    db: &'db dyn Db,
    symbol: &'db str,
    params: &'db [AbiType],
    ret_type: AbiReturn<'db>,
    ops: AbiFunctionRenderOps,
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
        let AbiFunctionRenderOps {
            transport,
            inject_js_env,
        } = ops;

        let boundary_type = fragment(move |p| match (ret_type, transport) {
            (AbiReturn::Value(type_), _) => transport.render_return_type(type_).apply(p),
            (AbiReturn::Union(_), AbiTransport::Wasm(_)) => "Vec<JsValue>".apply(p),
            (AbiReturn::Union(_), AbiTransport::Napi) => {
                "(u32, napi::bindgen_prelude::Either5<(), f64, bool, String, u64>)".apply(p);
            }
        });

        let definition = fragment(move |p| {
            if let AbiReturn::Union(union) = ret_type {
                let variants = comma_list(
                    union
                        .variants(db)
                        .iter()
                        .enumerate()
                        .map(|(index, type_)| frag!(V { index }({ type_.render_rust() }))),
                );

                frag!(enum __AbiReturn {{ {variants} }}@@).apply(p);
            }
        });

        let declarations = fragment(move |p| {
            if inject_js_env && matches!(transport, AbiTransport::Napi) {
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
                    AbiTransport::Wasm(_) => frag!(0).apply(p),
                    AbiTransport::Napi => frag!(__env.raw() as usize).apply(p),
                });

                frag!(
                    let __js_env = unsafe {{ rustier_rt::js::JsEnv::from_raw({raw}) }};@@
                )
                .apply(p);
            }

            for (i, type_) in params.iter().enumerate() {
                if *type_ == AbiType::Unit && matches!(transport, AbiTransport::Wasm(_)) {
                    frag!(let _{i} = ();@@).apply(p);
                    continue;
                }

                if *type_ != AbiType::usize {
                    continue;
                }

                let value = fragment(move |p| match transport {
                    AbiTransport::Wasm(_) => frag!(_{i}).apply(p),
                    AbiTransport::Napi => frag!(_{i}.get_u64().1).apply(p),
                });

                frag!(
                    let _{i} = {value}~ as usize;@@
                )
                .apply(p);
            }
        });

        let encode = fragment(move |p| match ret_type {
            AbiReturn::Value(AbiType::usize) => {
                let type_ = transport.render_return_type(AbiType::usize);

                frag!(result as ~{type_}).apply(p);
            }
            AbiReturn::Value(_) => "result".apply(p),
            AbiReturn::Union(union) => union.render_encode(db, transport).apply(p),
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

/// The logical arguments of an exported wrapper function.
#[derive(Clone, Copy)]
pub struct AbiArgs<'db> {
    params: &'db [AbiType],
}

impl<'db> AbiArgs<'db> {
    pub fn get(self, idx: usize) -> AbiArg {
        AbiArg {
            idx,
            type_: self.params[idx],
        }
    }
}

/// A logical argument of an exported wrapper function.
#[derive(Clone, Copy)]
pub struct AbiArg {
    idx: usize,
    type_: AbiType,
}

impl AbiArg {
    /// The logical ABI type of this argument.
    pub fn type_(self) -> AbiType {
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

    fn snapshot(name: &str, render: impl Fn(&dyn Db, AbiTransport) -> String) {
        let db = RootDatabase::default();
        let snapshot = fragment(|p| {
            for (label, transport) in [
                ("N-API", AbiTransport::Napi),
                (
                    "WASM32",
                    AbiTransport::Wasm(WasmTransport {
                        memory: WasmMemory::x32,
                    }),
                ),
                (
                    "WASM64",
                    AbiTransport::Wasm(WasmTransport {
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
                    ("unit", AbiType::Unit),
                    ("number", AbiType::f64),
                    ("boolean", AbiType::bool),
                    ("string", AbiType::String),
                    ("handle", AbiType::usize),
                ] {
                    let params = [type_];

                    FunctionFragment {
                        db,
                        symbol,
                        params: &params,
                        ret_type: AbiReturn::Value(type_),
                        ops: AbiFunctionRenderOps {
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
                ret_type: AbiReturn::Value(AbiType::Unit),
                ops: AbiFunctionRenderOps {
                    transport,
                    inject_js_env: true,
                },
                body: frag!(source::only_env(__js_env)),
            };
            let mixed_args = FunctionFragment {
                db,
                symbol: "mixed_args",
                params: &[AbiType::f64, AbiType::bool],
                ret_type: AbiReturn::Value(AbiType::f64),
                ops: AbiFunctionRenderOps {
                    transport,
                    inject_js_env: true,
                },
                body: frag!(source::mixed_args(_0, __js_env, _1)),
            };

            frag!({only_env}@@{mixed_args}).print()
        });
    }

    #[test]
    fn union_returns() {
        snapshot("union_returns", |db, transport| {
            let union = AbiUnion::new(
                db,
                vec![
                    AbiType::Unit,
                    AbiType::f64,
                    AbiType::bool,
                    AbiType::String,
                    AbiType::usize,
                    AbiType::String,
                    AbiType::Unit,
                ],
            );
            let ret_type = AbiReturn::Union(union);
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
                params: &[AbiType::f64],
                ret_type,
                ops: AbiFunctionRenderOps {
                    transport,
                    inject_js_env: false,
                },
                body,
            }
            .print()
        });
    }
}
