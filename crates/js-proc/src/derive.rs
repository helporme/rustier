use std::collections::{HashMap, HashSet};

use heck::{ToLowerCamelCase, ToSnakeCase};
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Attribute, Data, DeriveInput, Error, Fields, Generics, LitStr, Member, Result, Type,
    ext::IdentExt, parse_quote, spanned::Spanned, visit::Visit,
};

#[derive(Clone, Copy)]
pub enum Direction {
    Read,
    Write,
}

fn attrs(list: &[Attribute], allowed: &[&str]) -> Result<HashMap<String, String>> {
    let mut out = HashMap::new();
    for attr in list.iter().filter(|attr| attr.path().is_ident("js")) {
        attr.parse_nested_meta(|meta| {
            let key = meta
                .path
                .get_ident()
                .map(ToString::to_string)
                .unwrap_or_default();
            if !allowed.contains(&key.as_str()) {
                return Err(meta.error("unsupported #[js] attribute at this location"));
            }
            let value = match key.as_str() {
                "case" | "tag" | "rename" => meta.value()?.parse::<LitStr>()?.value(),
                _ => String::new(),
            };
            if out.insert(key, value).is_some() {
                return Err(meta.error("duplicate #[js] attribute"));
            }
            Ok(())
        })?;
    }
    Ok(out)
}

enum Case {
    Original,
    Snake,
    Camel,
}

impl Case {
    fn name(&self, ident: &Ident, rename: Option<&String>) -> String {
        rename.cloned().unwrap_or_else(|| {
            let name = ident.unraw().to_string();
            match self {
                Case::Original => name,
                Case::Snake => name.to_snake_case(),
                Case::Camel => name.to_lower_camel_case(),
            }
        })
    }
}

enum Mode {
    Convert,
    Skip,
    Handle,
}

struct Field<'a> {
    member: Member,
    key: String,
    binding: Ident,
    ty: &'a Type,
    mode: Mode,
}

struct Product<'a> {
    fields: Vec<Field<'a>>,
    positional: bool,
}

impl<'a> Product<'a> {
    fn parse(fields: &'a Fields, case: &Case, reserved: Option<&str>) -> Result<Self> {
        let positional = matches!(fields, Fields::Unnamed(_));
        let mut names = HashSet::new();
        if !positional {
            names.extend(reserved.map(str::to_owned));
        }
        if positional && reserved == Some("values") {
            return Err(Error::new(
                fields.span(),
                "tuple variant payload collides with tag `values`",
            ));
        }
        let mut layout = Vec::new();
        let mut index = 0;
        for (i, field) in fields.iter().enumerate() {
            let attrs = attrs(&field.attrs, &["skip", "rename", "handle"])?;
            let mode = match (attrs.contains_key("skip"), attrs.contains_key("handle")) {
                (true, true) => {
                    return Err(Error::new(
                        field.span(),
                        "`skip` and `handle` cannot be combined",
                    ));
                }
                (true, false) => Mode::Skip,
                (false, true) => Mode::Handle,
                (false, false) => Mode::Convert,
            };
            let (member, key) = if let Some(id) = &field.ident {
                (
                    Member::Named(id.clone()),
                    case.name(id, attrs.get("rename")),
                )
            } else {
                if attrs.contains_key("rename") {
                    return Err(Error::new(field.span(), "cannot rename a positional field"));
                }
                (Member::Unnamed(i.into()), index.to_string())
            };
            if !matches!(mode, Mode::Skip) {
                if !names.insert(key.clone()) {
                    return Err(Error::new(
                        field.span(),
                        format!("duplicate JS field `{key}`"),
                    ));
                }
                index += 1;
            }
            layout.push(Field {
                member,
                key,
                binding: format_ident!("__field_{i}"),
                ty: &field.ty,
                mode,
            });
        }
        Ok(Self {
            fields: layout,
            positional,
        })
    }

    fn add_bounds(&self, name: &Ident, generics: &mut Generics, dir: Direction, rt: &TokenStream) {
        if generics.params.is_empty() {
            return;
        }
        for Field { ty, mode, .. } in &self.fields {
            let bound = match (mode, dir) {
                (Mode::Skip, Direction::Write) => continue,
                (Mode::Skip, Direction::Read) => quote!(::core::default::Default),
                (Mode::Handle, _) => quote!('static),
                (_, Direction::Read) => quote!(#rt::FromJs),
                (_, Direction::Write) => quote!(#rt::ToJs),
            };
            // A predicate such as `Vec<Tree<T>>: FromJs` would require the
            // very impl being derived. Only constrain independent parameters
            // in such a field; the recursive conversion uses this impl.
            if matches!(mode, Mode::Convert) {
                let mut refs = FieldTypes {
                    container: name,
                    recursive: false,
                    params: generics.type_params().map(|p| &p.ident).collect(),
                    used: Vec::new(),
                };
                refs.visit_type(ty);
                if refs.recursive {
                    let used = refs.used;
                    for param in used {
                        generics
                            .make_where_clause()
                            .predicates
                            .push(parse_quote!(#param: #bound));
                    }
                    continue;
                }
            }
            generics
                .make_where_clause()
                .predicates
                .push(parse_quote!(#ty: #bound));
        }
    }

    fn write(&self, rt: &TokenStream, tag: Option<(&str, &str)>) -> TokenStream {
        let init = if self.positional {
            quote!(#rt::__convert::array(env)?)
        } else {
            quote!(env.new_object())
        };
        let puts = self.fields.iter().filter_map(
            |Field {
                 key,
                 binding,
                 ty,
                 mode,
                 ..
             }| {
                let value = match mode {
                    Mode::Skip => return None,
                    Mode::Handle => quote!(#rt::__convert::write_handle(#binding, env)?),
                    Mode::Convert => quote!(<#ty as #rt::ToJs>::to_js(#binding, env)?),
                };
                Some(quote!(#rt::__convert::put(&out, #key, #value)?;))
            },
        );
        let wrap = self.positional.then(|| {
            quote!(
                let values = out;
                let out = env.new_object();
                #rt::__convert::put(&out, "values", values)?;
            )
        });
        let tagged = tag.map(|(key, name)| {
            quote!(
                #wrap
                #rt::__convert::put(&out, #key, env.new_string(#name))?;
            )
        });
        quote!({
            let out = #init;
            #(#puts)*
            #tagged
            Ok(out)
        })
    }

    fn pattern(&self, ctor: &TokenStream) -> TokenStream {
        let fields = self.fields.iter().map(
            |Field {
                 member,
                 binding,
                 mode,
                 ..
             }| match mode {
                Mode::Skip => quote!(#member: _),
                _ => quote!(#member: #binding),
            },
        );
        quote!(#ctor { #(#fields),* })
    }

    fn read(&self, ctor: &TokenStream, rt: &TokenStream) -> TokenStream {
        let check = if self.positional {
            let len = self
                .fields
                .iter()
                .filter(|f| !matches!(f.mode, Mode::Skip))
                .count();
            quote!(#rt::__convert::check_length(&value, #len)?;)
        } else {
            quote!(#rt::__convert::check_object(&value)?;)
        };
        let fields = self.fields.iter().map(
            |Field {
                 member,
                 key,
                 ty,
                 mode,
                 ..
             }| {
                let expr = match mode {
                    Mode::Skip => quote!(::core::default::Default::default()),
                    Mode::Handle => quote!(#rt::__convert::read_handle::<#ty>(&value, #key)?),
                    Mode::Convert if self.positional => quote!(<#ty as #rt::FromJs>::from_js(
                    #rt::__convert::own_field(&value, #key)?.ok_or_else(||
                        #rt::JsCallError::new(concat!("missing array element ", #key))
                    )?
                )?),
                    Mode::Convert => quote!(#rt::__convert::read_field::<#ty>(&value, #key)?),
                };
                quote!(#member: #expr)
            },
        );
        quote!({ #check Ok(#ctor { #(#fields),* }) })
    }
}

struct FieldTypes<'a> {
    container: &'a Ident,
    recursive: bool,
    params: HashSet<&'a Ident>,
    used: Vec<Ident>,
}

impl<'ast> Visit<'ast> for FieldTypes<'_> {
    fn visit_type_path(&mut self, ty: &'ast syn::TypePath) {
        let last = &ty.path.segments.last().unwrap().ident;
        if last == "Self" || last == self.container {
            self.recursive = true;
            return;
        }
        let first = &ty.path.segments.first().unwrap().ident;
        if self.params.contains(first) {
            self.used.push(first.clone());
        }
        syn::visit::visit_type_path(self, ty)
    }
}

pub fn expand(input: DeriveInput, dir: Direction) -> Result<TokenStream> {
    let rt = match crate_name("rustier") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let id = format_ident!("{name}");
            quote!(::#id)
        }
        // rust-analyzer's proc-macro server may not have a Cargo manifest.
        Err(_) => quote!(::rustier),
    };
    let options = attrs(
        &input.attrs,
        if matches!(input.data, Data::Enum(_)) {
            &["case", "tag"]
        } else {
            &["case"]
        },
    )?;
    let case = match options.get("case").map(String::as_str) {
        None => Case::Original,
        Some("snake_case") => Case::Snake,
        Some("camelCase") => Case::Camel,
        _ => {
            return Err(Error::new(
                input.ident.span(),
                "expected `snake_case` or `camelCase`",
            ));
        }
    };
    let mut generics = input.generics;
    let ident = input.ident;
    let body = match input.data {
        Data::Union(_) => {
            return Err(Error::new(
                ident.span(),
                "JS conversion requires a struct or enum",
            ));
        }
        Data::Struct(data) => {
            let product = Product::parse(&data.fields, &case, None)?;
            product.add_bounds(&ident, &mut generics, dir, &rt);
            match dir {
                Direction::Read => product.read(&quote!(Self), &rt),
                Direction::Write => {
                    let pattern = product.pattern(&quote!(Self));
                    let write = product.write(&rt, None);
                    quote!(let #pattern = self; #write)
                }
            }
        }
        Data::Enum(data) => {
            let tagged = data
                .variants
                .iter()
                .any(|v| !matches!(v.fields, Fields::Unit));
            let tag = options.get("tag").map(String::as_str).unwrap_or("type");
            let mut names = HashSet::new();
            let mut arms = Vec::new();
            for v in &data.variants {
                let name = case.name(&v.ident, attrs(&v.attrs, &["rename"])?.get("rename"));
                if !names.insert(name.clone()) {
                    return Err(Error::new(
                        v.ident.span(),
                        format!("duplicate JS variant `{name}`"),
                    ));
                }
                let variant = &v.ident;
                let ctor = quote!(Self::#variant);
                let product = Product::parse(&v.fields, &case, tagged.then_some(tag))?;
                product.add_bounds(&ident, &mut generics, dir, &rt);
                arms.push(match (dir, tagged) {
                    (Direction::Write, false) => quote!(#ctor => Ok(env.new_string(#name))),
                    (Direction::Read, false) => quote!(#name => Ok(#ctor)),
                    (Direction::Write, true) => {
                        let pat = product.pattern(&ctor);
                        let write = product.write(&rt, Some((tag, &name)));
                        quote!(#pat => #write)
                    }
                    (Direction::Read, true) => {
                        let payload = product.positional.then(|| quote!(
                            let value = #rt::__convert::own_field(&value, "values")?
                                .ok_or_else(|| #rt::JsCallError::new("missing tuple variant payload"))?;
                        ));
                        let read = product.read(&ctor, &rt);
                        quote!(#name => { #payload #read })
                    }
                });
            }
            match dir {
                Direction::Write => quote!(match self { #(#arms,)* }),
                Direction::Read => {
                    let discriminant = if tagged {
                        quote!(#rt::__convert::own_field(&value, #tag)?.and_then(|v| v.to_string()))
                    } else {
                        quote!(value.to_string())
                    };
                    quote!(match #discriminant.ok_or_else(|| #rt::JsCallError::new("expected a string enum tag"))?.as_str() {
                        #(#arms,)*
                        unknown => Err(#rt::JsCallError::new(format!("unknown {} variant `{unknown}`", stringify!(#ident)))),
                    })
                }
            }
        }
    };
    let (imp, args, where_) = generics.split_for_impl();
    Ok(match dir {
        Direction::Read => quote!(
            impl #imp #rt::FromJs for #ident #args #where_ {
                fn from_js(value: #rt::JsValue) -> ::core::result::Result<Self, #rt::JsCallError> {
                    #body
                }
            }
        ),
        Direction::Write => quote!(
            impl #imp #rt::ToJs for #ident #args #where_ {
                fn to_js(self, env: #rt::JsEnv) -> ::core::result::Result<#rt::JsValue, #rt::JsCallError> {
                    #body
                }
            }
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_ambiguous_wire_shapes_in_either_direction() {
        for source in [
            r#"struct S { a: u8, #[js(rename = "a")] b: u8 }"#,
            r#"enum E { A { #[js(rename = "type")] name: String } }"#,
            r#"#[js(tag = "values")] enum E { A(u8) }"#,
            r#"enum E { A, #[js(rename = "A")] B }"#,
            r#"struct S(#[js(opaque)] u8);"#,
            r#"struct S(#[js(handle, skip)] u8);"#,
        ] {
            for dir in [Direction::Read, Direction::Write] {
                assert!(
                    expand(syn::parse_str(source).unwrap(), dir).is_err(),
                    "{source}"
                );
            }
        }
    }
}
