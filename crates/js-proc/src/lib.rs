use proc_macro::TokenStream;
use quote::{format_ident, quote};
use rustier_js_syntax::ParsedJsMacro;

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
