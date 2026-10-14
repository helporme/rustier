//! Rust side of the ABI: exported wrapper functions.

use crate::compiler::{
    codegen::fragment::{FragmentOnce, Printer, comma_list, doubles, frag, fragment},
    db::Db,
    ir::abi::{
        AbiFunction, AbiType,
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

    fn render_result_type<'a>(self, ok_type: impl FragmentOnce + 'a) -> impl FragmentOnce + 'a {
        fragment(move |p| match self {
            AbiTransport::Wasm(_) => frag!(Result<{ok_type}, JsValue>).apply(p),
            AbiTransport::Napi => frag!(napi::Result<{ ok_type }>).apply(p),
        })
    }

    fn render_error<'a>(self, error: impl FragmentOnce + 'a) -> impl FragmentOnce + 'a {
        fragment(move |p| match self {
            AbiTransport::Wasm(_) => frag!(JsValue::from_str(&{ error }.to_string())).apply(p),
            AbiTransport::Napi => frag!(napi::Error::from_reason({ error }.to_string())).apply(p),
        })
    }

    fn render_param_type(self, type_: AbiType) -> impl FragmentOnce {
        fragment(move |p| match (type_, self) {
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
            AbiType::f64 => "f64",
            AbiType::bool => "bool",
            AbiType::String => "String",
            AbiType::usize => "usize",
        }
    }
}

impl<'db> AbiFunction<'db> {
    /// Renders the exported wrapper of this function.
    ///
    /// The body receives the logical arguments of the function and must
    /// evaluate to a `rustier_rt::CallResult` of the Rust form of the return
    /// type. It may use `?` and `return` on runtime errors; the wrapper
    /// converts them into the transport's error convention.
    pub fn render_rust_with_body<Body>(
        self,
        db: &'db dyn Db,
        transport: AbiTransport,
        body: impl FnOnce(AbiArgs<'db>) -> Body + 'db,
    ) -> impl FragmentOnce + 'db
    where
        Body: FragmentOnce + 'db,
    {
        let params = self.params(db);

        FunctionFragment {
            symbol: self.symbol(db),
            params,
            ret_type: self.ret_type(db),
            transport,
            body: body(AbiArgs { params }),
        }
    }
}

struct FunctionFragment<'db, Body> {
    symbol: &'db str,
    params: &'db [AbiType],
    ret_type: Option<AbiType>,
    transport: AbiTransport,
    body: Body,
}

impl<'db, Body: FragmentOnce + 'db> FragmentOnce for FunctionFragment<'db, Body> {
    fn apply(self, p: &mut Printer) {
        let Self {
            symbol,
            params,
            ret_type,
            transport,
            body,
        } = self;

        let body_ok_type = || {
            fragment(move |p| match ret_type {
                Some(type_) => type_.render_rust().apply(p),
                None => "()".apply(p),
            })
        };

        let boundary_ok_type = fragment(move |p| match ret_type {
            Some(type_) => transport.render_return_type(type_).apply(p),
            None => "()".apply(p),
        });

        let declarations = comma_list(params.iter().enumerate().map(|(i, type_)| {
            frag!(
                _{i}: ~{transport.render_param_type(*type_)}
            )
        }));

        let decode = fragment(move |p| {
            for (i, type_) in params.iter().enumerate() {
                if *type_ != AbiType::usize {
                    continue;
                }

                let error =
                    || transport.render_error(doubles("ABI value is outside the usize range"));

                let value = fragment(move |p| match transport {
                    AbiTransport::Wasm(_) => frag!(_{i}).apply(p),
                    AbiTransport::Napi => frag!({{
                        let (negative, value, lossless) = _{i}.get_u64();@@

                        if negative || !lossless {{
                            return Err({error()});
                        }}@@

                        value
                    }})
                    .apply(p),
                });

                frag!(
                    let _{i} = usize::try_from({value})
                        .map_err(|_| {error()})?;@@
                )
                .apply(p);
            }
        });

        let encode = fragment(move |p| {
            if ret_type == Some(AbiType::usize) {
                let type_ = transport.render_return_type(AbiType::usize);
                let error =
                    transport.render_error(doubles("usize value is outside the transport range"));

                frag!(
                    .and_then(|result| {type_}::try_from(result).map_err(|_| {error}))
                )
                .apply(p);
            }
        });

        frag!(
            {transport.render_header(symbol)}@
            pub fn ~{symbol}({declarations})~ -> ~{transport.render_result_type(boundary_ok_type)}~ {{
                {decode}
                let result = (|| -> rustier_rt::CallResult<{body_ok_type()}>~{{
                    {body}
                }})();@@

                result.map_err(|error| {transport.render_error("error")}){encode}
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
