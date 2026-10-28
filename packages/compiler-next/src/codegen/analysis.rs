//! Generates the Rust closure used to infer an `rs` macro's signature.
//! This source is analyzed in memory, not emitted into the bridge crate.

use crate::{
    db::Db,
    ir::runtime::{RsMacro, RsPart, RsType},
};

use super::fragment::{FragmentOnce, FragmentPrint, frag, fragment};

impl<'db> RsMacro<'db> {
    /// Renders the macro body with one closure parameter per interpolation.
    pub(crate) fn render_analysis_rs(self, db: &'db dyn Db, types: Option<&[RsType]>) -> String {
        let parts = self.body(db);
        let params = fragment(|p| {
            for (index, _) in parts
                .iter()
                .filter(|part| matches!(part, RsPart::Input))
                .enumerate()
            {
                if index != 0 {
                    frag!(, ~).apply(p);
                }

                frag!(__rustier_arg_ { index }).apply(p);

                if let Some(types) = types {
                    let type_ = types[index].render();
                    frag!(: ~{type_}).apply(p);
                }
            }
        });

        let body = fragment(|p| {
            let mut index = 0;

            for part in parts {
                match part {
                    RsPart::Text(text) => text.apply(p),
                    RsPart::Input => {
                        frag!(__rustier_arg_ { index }).apply(p);
                        index += 1;
                    }
                }
            }
        });

        frag!(
            fn __rustier_probe(__rustier_env: ::aux_next::runtime::js::JsEnv) {{
                let __rustier_snippet = |{params}| {{
                    {body}@
                }};@
            }}
        )
        .print()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::RootDatabase;

    fn snapshot(name: &str, parts: Vec<RsPart>, types: Option<&[RsType]>) {
        let db = RootDatabase::default();
        let macro_ = RsMacro::new(&db, parts);
        let source = macro_.render_analysis_rs(&db, types);
        let rust = prettyplease::unparse(&syn::parse_file(&source).unwrap());
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
