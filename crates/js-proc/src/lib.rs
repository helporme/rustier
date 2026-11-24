use proc_macro::TokenStream;
use quote::{format_ident, quote};
use rustier_js_syntax::ParsedJsMacro;

mod derive;

/// Implements `rustier::ToJs` for a struct or enum.
///
/// # Example
///
/// ```
/// use rustier::{JsCallError, JsEnv, JsValue, ToJs};
///
/// #[derive(ToJs)]
/// #[js(case = "camelCase")]
/// struct Cat {
///     #[js(handle)]
///     pet_name: String,
///     #[js(rename = "_age")]
///     age: u8,
///     #[js(skip)]
///     cached_weight: Option<f64>,
/// }
///
/// fn cat(env: JsEnv) -> Result<JsValue, JsCallError> {
///     // Produces { petName: <handle of rust value>, _age: 1 }
///     Cat { pet_name: "Fluffy".into(), age: 1, cached_weight: None }.to_js(env)
/// }
/// ```
///
/// # Attributes
///
/// - `#[js(case = "camelCase")]` or `#[js(case = "snake_case")]`
///
///   On a struct or enum, write named fields and variants in the selected case.
///   Without this attribute, keep their Rust names.
///
/// - `#[js(tag = "kind")]`
///
///   On an enum with data, write the variant name to `"kind"` instead of `"type"`.
///
/// - `#[js(rename = "name")]`
///
///   Write a named field or variant with this exact name, overriding `case`.
///   Tuple fields cannot be renamed.
///
/// - `#[js(skip)]`
///
///   Omit this field.
///
/// - `#[js(handle)]`
///
///   Move this field value into js as handle of rust value. The field type must be `'static`.
#[proc_macro_derive(ToJs, attributes(js))]
pub fn derive_to_js(input: TokenStream) -> TokenStream {
    derive::expand(syn::parse_macro_input!(input), derive::Direction::Write)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Implements `rustier::FromJs` for a struct or enum.
///
/// # Example
///
/// ```
/// use rustier::{FromJs, JsCallError, JsValue};
///
/// #[derive(FromJs)]
/// #[js(tag = "kind")]
/// enum Batch<T> {
///     Empty,
///     #[js(rename = "one")]
///     One(#[js(handle)] T),
///     Many { items: Vec<T> },
/// }
///
/// fn read_batch(value: JsValue) -> Result<Batch<u8>, JsCallError> {
///     // { kind: "one", values: [<handle of 8>] } bcomes Batch::One(8)
///     // { kind: "Many", items: [7, 8] } becomes Batch::Many { items: vec![7, 8] }.
///     Batch::from_js(value)
/// }
/// ```
///
/// # Attributes
///
/// - `#[js(case = "camelCase")]` or `#[js(case = "snake_case")]`
///
///   On a struct or enum, match named fields and variants in the selected case.
///   Without this attribute, match their Rust names.
///
/// - `#[js(tag = "kind")]`
///
///   On an enum with data, read the variant name from `"kind"` instead of `"type"`.
///
/// - `#[js(rename = "name")]`
///
///   Match a named field or variant by this exact name, overriding `case`.
///
/// - `#[js(skip)]`
///
///   Initialize this field with `Default::default()` instead of reading a property
///   or array element. The field type needs to implement `Default`.
///
/// - `#[js(handle)]`
///
///   Read this field value from a handle of rust value. The field type must be `'static`.
#[proc_macro_derive(FromJs, attributes(js))]
pub fn derive_from_js(input: TokenStream) -> TokenStream {
    derive::expand(syn::parse_macro_input!(input), derive::Direction::Read)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Expands a deferred call using the runtime crate supplied by `js!`.
#[doc(hidden)]
#[proc_macro]
pub fn __expand_js(tokens: TokenStream) -> TokenStream {
    let mut tokens = proc_macro2::TokenStream::from(tokens).into_iter();
    let Some(runtime) = tokens.next() else {
        return quote!(compile_error!("missing Rustier runtime")).into();
    };
    expand(quote!(#runtime), tokens.collect()).into()
}

/// Executes `js!` using the environment supplied by the enclosing `rs` call.
#[doc(hidden)]
#[proc_macro]
pub fn __js_in_rs(tokens: TokenStream) -> TokenStream {
    let call = expand(quote!(::rustier_runtime), tokens.into());
    quote!(#call.at(__rustier_env)).into()
}

fn expand(
    runtime: proc_macro2::TokenStream,
    tokens: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let mut tokens = tokens.into_iter().peekable();
    let analyzed = tokens.peek().and_then(|token| match token {
        proc_macro2::TokenTree::Literal(literal) => match literal.to_string().as_str() {
            "0__rustier_output" => Some(false),
            "1__rustier_output" => Some(true),
            _ => None,
        },
        _ => None,
    });
    if analyzed.is_some() {
        tokens.next();
    }
    let ParsedJsMacro { body, inputs } = match ParsedJsMacro::parse(tokens.collect()) {
        Ok(snippet) => snippet,
        Err(message) => return quote!(compile_error!(#message)),
    };
    let boundary = format_ident!("{}", rustier_js_syntax::symbol(&body));
    let (output, convert) = if analyzed.unwrap_or_else(|| rustier_js_syntax::returns_value(&body)) {
        (quote!(#runtime::JsValue), quote!())
    } else {
        (quote!(()), quote!(.map(::core::mem::drop)))
    };
    let args: Vec<_> = (0..inputs.len())
        .map(|i| format_ident!("arg_{i}"))
        .collect();
    quote!(({
        struct Call { #(#args: #runtime::js_call::Input),* }

        unsafe extern "Rust" {
            fn #boundary(env: #runtime::JsEnv, #(#args: #runtime::js_call::Input),*)
                -> ::std::result::Result<#runtime::JsValue, #runtime::JsCallError>;
        }

        impl #runtime::JsMacro for Call {
            type Output = #output;

            fn at(self, env: #runtime::JsEnv) -> ::std::result::Result<Self::Output, #runtime::JsCallError> {
                unsafe { #boundary(env, #(self.#args),*) } #convert
            }
        }
        impl Call {
            fn at(self, env: #runtime::JsEnv) -> ::std::result::Result<#output, #runtime::JsCallError> {
                #runtime::JsMacro::at(self, env)
            }
        }
        |#(#args),*| {
            if false {
                fn assert_static<T: 'static>(_: &T) {}
                #(assert_static(&#args);)*
            }
            Call { #(#args: #runtime::js_call::Input::new(#args)),* }
        }
    })(#({ #inputs }),*))
}
