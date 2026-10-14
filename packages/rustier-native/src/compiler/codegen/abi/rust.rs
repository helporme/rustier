//! Rust side of the ABI: exported wrapper functions and carrier classes.

use crate::compiler::{
    codegen::fragment::{FragmentOnce, comma_list, doubles, frag, fragment},
    db::Db,
    ir::abi::{AbiClass, AbiFunction, AbiType, transport::AbiTransport},
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
}

impl<'db> AbiType<'db> {
    /// The Rust type a value of this ABI type has at the transport boundary.
    pub fn render_rust(self, db: &'db dyn Db) -> impl FragmentOnce + 'db {
        fragment(move |p| match self {
            AbiType::F64 => "f64".apply(p),
            AbiType::Bool => "bool".apply(p),
            AbiType::String => "String".apply(p),
            AbiType::Class {
                class,
                borrowed: true,
            } => frag!(&{ class.symbol(db) }).apply(p),
            AbiType::Class {
                class,
                borrowed: false,
            } => class.symbol(db).apply(p),
        })
    }
}

impl<'db> AbiClass<'db> {
    /// Renders the transport-decorated carrier struct of this class with the
    /// given fields.
    // FIXME: remove once classes are transferred as handles.
    pub fn render_rust_with_fields(
        self,
        db: &'db dyn Db,
        transport: AbiTransport,
        fields: impl FragmentOnce + 'db,
    ) -> impl FragmentOnce + 'db {
        let symbol = self.symbol(db);

        frag!(
            {transport.render_header(symbol)}
            pub struct ~{symbol}~{{
                {fields}
            }}
        )
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
        fragment(move |p| {
            let ok_type = || {
                fragment(move |p| match self.ret_type(db) {
                    Some(type_) => type_.render_rust(db).apply(p),
                    None => "()".apply(p),
                })
            };

            let params = comma_list(self.params(db).iter().enumerate().map(|(i, type_)| {
                frag!(
                    _{i}: ~{type_.render_rust(db)}
                )
            }));

            let symbol = self.symbol(db);
            let body = body(AbiArgs {
                params: self.params(db),
            });

            frag!(
                {transport.render_header(symbol)}@
                pub fn ~{symbol}({params})~ -> ~{transport.render_result_type(ok_type())}~ {{
                    let result = (|| -> rustier_rt::CallResult<{ok_type()}>~{{
                        {body}
                    }})();@@

                    result.map_err(|error| {transport.render_error("error")})
                }}
            )
            .apply(p);
        })
    }
}

/// The logical arguments of an exported wrapper function.
#[derive(Clone, Copy)]
pub struct AbiArgs<'db> {
    params: &'db [AbiType<'db>],
}

impl<'db> AbiArgs<'db> {
    pub fn get(self, idx: usize) -> AbiArg<'db> {
        AbiArg {
            idx,
            type_: self.params[idx],
        }
    }
}

/// A logical argument of an exported wrapper function.
#[derive(Clone, Copy)]
pub struct AbiArg<'db> {
    idx: usize,
    type_: AbiType<'db>,
}

impl<'db> AbiArg<'db> {
    /// The logical ABI type of this argument.
    pub fn type_(self) -> AbiType<'db> {
        self.type_
    }

    /// An expression evaluating to the value of this argument.
    pub fn expr(self) -> impl FragmentOnce + Copy {
        let idx = self.idx;

        frag!(_{idx})
    }
}
