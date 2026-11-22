use oxc_ast::ast::{FunctionBody, ReturnStatement};
use oxc_ast_visit::Visit;
use proc_macro2::{Delimiter, LineColumn, TokenStream, TokenTree};

pub struct ParsedJsMacro {
    pub body: String,
    pub inputs: Vec<TokenStream>,
}

impl ParsedJsMacro {
    pub fn parse(stream: TokenStream) -> Result<Self, &'static str> {
        let expression = matches!(
            stream.clone().into_iter().collect::<Vec<_>>().as_slice(),
            [TokenTree::Group(group)] if group.delimiter() == Delimiter::Brace
        );
        let mut inputs = Vec::new();
        let mut body = String::new();
        render(stream, &mut body, &mut inputs)?;
        if expression {
            body = format!("return ({body});");
        }
        Ok(Self { body, inputs })
    }
}

/// Stable Rust link name for a rendered JavaScript body with positional inputs.
pub fn symbol(body: &str) -> String {
    format!("__rustier_js_{}", blake3::hash(body.as_bytes()).to_hex())
}

/// Whether the rendered body has a value-bearing return outside nested functions.
pub fn returns_value(body: &str) -> bool {
    struct ReturnsValue(bool);

    impl<'a> Visit<'a> for ReturnsValue {
        fn visit_function_body(&mut self, _: &FunctionBody<'a>) {}

        fn visit_return_statement(&mut self, statement: &ReturnStatement<'a>) {
            self.0 |= statement.argument.is_some();
        }
    }

    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, body, oxc_span::SourceType::mjs())
        .with_options(oxc_parser::ParseOptions {
            allow_return_outside_function: true,
            ..Default::default()
        })
        .parse();
    let mut visitor = ReturnsValue(false);
    visitor.visit_program(&parsed.program);
    visitor.0
}

fn render(
    stream: TokenStream,
    body: &mut String,
    inputs: &mut Vec<TokenStream>,
) -> Result<(), &'static str> {
    let mut tokens = stream.into_iter();
    let mut end: Option<LineColumn> = None;
    while let Some(token) = tokens.next() {
        let start = token.span().start();
        if end.is_some_and(|end| end.line < start.line) {
            body.push('\n');
        } else if end.is_some_and(|end| end != start) {
            body.push(' ');
        }
        end = Some(token.span().end());
        match token {
            TokenTree::Punct(p) if p.as_char() == '@' => {
                let Some(TokenTree::Group(group)) = tokens.next() else {
                    return Err("expected @{Rust expression}");
                };
                if group.delimiter() != Delimiter::Brace || group.stream().is_empty() {
                    return Err("expected @{Rust expression}");
                }
                end = Some(group.span().end());
                body.push_str(&format!("(__rustier_input_{})", inputs.len()));
                inputs.push(group.stream());
            }
            TokenTree::Group(group) => {
                let (left, right) = match group.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::None => ("", ""),
                };
                body.push_str(left);
                render(group.stream(), body, inputs)?;
                body.push_str(right);
            }
            TokenTree::Ident(id) if id.to_string().starts_with("__rustier_input_") => {
                return Err("reserved js! parameter name");
            }
            other => body.push_str(&other.to_string()),
        }
    }
    Ok(())
}
