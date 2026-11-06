//! In-memory analysis probes and their mapping to macro body parts.

use crate::{
    db::Db,
    ir::runtime::{RsMacro, RsPart, RsType},
};

use ra_ap_syntax::{
    AstNode, Edition, SourceFile, TextRange, TextSize,
    ast::{self, HasModuleItem, HasName},
};

use super::fragment::{FragmentOnce, FragmentPrint, frag, fragment};

pub(crate) struct AnalysisSource {
    pub text: String,
    pub source_map: Vec<(TextRange, usize)>,
}

impl AnalysisSource {
    fn new(text: String, parts: &[RsPart]) -> Self {
        let parsed = SourceFile::parse(&text, Edition::CURRENT);
        let mut source_map = Vec::new();

        for item in parsed.tree().items() {
            let ast::Item::Fn(function) = item else {
                continue;
            };
            if !function
                .name()
                .is_some_and(|name| matches!(name.text(), "__rustier_probe" | "__rustier_check"))
            {
                continue;
            }

            let Some(closure) = function
                .syntax()
                .descendants()
                .find_map(ast::ClosureExpr::cast)
            else {
                continue;
            };
            let Some(ast::Expr::BlockExpr(body)) = closure.body() else {
                continue;
            };
            let Some(open) = body.stmt_list().and_then(|body| body.l_curly_token()) else {
                continue;
            };
            let Some(parameters) = closure.param_list() else {
                continue;
            };
            let mut parameters = parameters.params();
            let mut offset = open.text_range().end();

            for (part, value) in parts.iter().enumerate() {
                let len = match value {
                    RsPart::Text(text) => TextSize::of(text),
                    RsPart::Input => {
                        let Some(param) = parameters.next() else {
                            break;
                        };
                        let Some(name) = param.pat() else { break };
                        source_map.push((param.syntax().text_range(), part));
                        name.syntax().text_range().len()
                    }
                };

                source_map.push((TextRange::at(offset, len), part));
                offset += len;
            }
        }

        Self { text, source_map }
    }
}

impl<'db> RsMacro<'db> {
    pub(crate) fn render_analysis_rs(
        self,
        db: &'db dyn Db,
        types: Option<&[RsType]>,
        output: Option<RsType>,
    ) -> AnalysisSource {
        let fallible_check = output.is_some() && self.can_return_error(db);
        let text = fragment(|p| {
            for (index, output) in std::iter::once(None).chain(output.map(Some)).enumerate() {
                if index == 0 && fallible_check {
                    continue;
                }
                let name = if index == 0 {
                    "__rustier_probe"
                } else {
                    "__rustier_check"
                };
                frag!(
                    fn ~{name}(__rustier_env: ::aux_next::runtime::js::JsEnv) ~{"{"}@
                    let __rustier_snippet = |
                )
                .apply(p);

                for (input, _) in self
                    .body(db)
                    .iter()
                    .filter(|value| matches!(value, RsPart::Input))
                    .enumerate()
                {
                    if input != 0 {
                        frag!(, ~).apply(p);
                    }

                    let annotation = types.map(|types| frag!(: ~{types[input].render()}));
                    frag!(__rustier_arg_{input}{annotation}).apply(p);
                }

                let output = output.map(|ty| {
                    fragment(move |p| {
                        frag!(~-> ~).apply(p);
                        if self.can_return_error(db) {
                            self.render_result_type(db).apply(p);
                        } else {
                            ty.render().apply(p);
                        }
                    })
                });
                frag!(|{output} ~{"{"}).apply(p);
                let mut input = 0;

                for value in self.body(db) {
                    match value {
                        RsPart::Text(text) => text.apply(p),
                        RsPart::Input => {
                            frag!(__rustier_arg_ { input }).apply(p);
                            input += 1;
                        }
                    }
                }

                frag!(@{"};"}@).apply(p);

                if fallible_check {
                    let inputs = fragment(|p| {
                        for ty in types.unwrap() {
                            frag!({ty.render()},).apply(p);
                        }
                    });
                    let value = self.ret_type(db).render();

                    frag!(
                        fn __rustier_check_error<E, F>(_: F)
                        where
                            E: Into<::std::boxed::Box<dyn ::std::error::Error + Send + Sync + 'static>>,
                            F: FnOnce({inputs}) -> ::std::result::Result<{value}, E>
                        {{}}@
                        __rustier_check_error(__rustier_snippet);@
                    ).apply(p);
                }

                frag!({"}"}@).apply(p);
            }
        })
        .print();

        AnalysisSource::new(text, self.body(db))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::RootDatabase;

    fn snapshot(name: &str, parts: Vec<RsPart>, types: Option<&[RsType]>) {
        let db = RootDatabase::default();
        let macro_ = RsMacro::new(&db, parts);
        let source = macro_.render_analysis_rs(&db, types, None);
        let rust = prettyplease::unparse(&syn::parse_file(&source.text).unwrap());
        let output = format!("```rust\n{rust}```\n");

        insta::with_settings!({
            snapshot_path => "_snapshots/analysis",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!(name, output);
        });
    }

    #[test]
    fn untyped_inputs() {
        snapshot(
            "untyped_inputs",
            vec![
                RsPart::Text("let first = ".into()),
                RsPart::Input,
                RsPart::Text("; let second = ".into()),
                RsPart::Input,
                RsPart::Text("; second".into()),
            ],
            None,
        );
    }

    #[test]
    fn typed_inputs() {
        snapshot(
            "typed_inputs",
            vec![
                RsPart::Text("let _: () = ".into()),
                RsPart::Input,
                RsPart::Text("; ".into()),
                RsPart::Input,
            ],
            Some(&[RsType::Void, RsType::JsValue]),
        );
    }

    #[test]
    fn no_inputs() {
        snapshot("no_inputs", vec![RsPart::Text("()".into())], None);
    }
}
