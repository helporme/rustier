use std::fmt::Write as _;

use crate::{
    codegen::fragment::{FragmentOnce, FragmentPrint, doubles, frag, fragment},
    db::Db,
    ir::js::{JsFileRange, JsModule, JsPart, RsMacroSrc},
};

#[derive(Clone, PartialEq, Eq, salsa::Update)]
pub struct JsEdit {
    pub start: u32,
    pub end: u32,
    pub text: String,
}

#[salsa::tracked]
impl<'db> JsModule<'db> {
    /// Replaces imported `rs` tags with calls to their exported macro functions.
    #[salsa::tracked(returns(ref))]
    pub fn render_transformed(self, db: &'db dyn Db, ffi_js_mod_path: String) -> String {
        let source = self.file(db).contents(db);

        fragment(|p| {
            let mut cursor = 0;

            for edit in self.render_edits(db, ffi_js_mod_path) {
                source[cursor..edit.start as usize].apply(p);
                edit.text.as_str().apply(p);
                cursor = edit.end as usize;
            }

            source[cursor..].apply(p);
        })
        .print()
    }

    #[salsa::tracked(returns(ref))]
    pub fn render_edits(self, db: &'db dyn Db, ffi_js_mod_path: String) -> Vec<JsEdit> {
        let file = self.file(db);
        let macros = self.macros(db);
        let source = file.contents(db);

        if macros.is_empty() {
            return Vec::new();
        }

        let mut binding = "__rustier_ffi".to_owned();
        while source.contains(&binding) {
            binding.push('_');
        }

        let start = if source.starts_with("#!") {
            source.find('\n').map_or(source.len(), |index| index + 1)
        } else {
            0
        };
        let path = doubles(ffi_js_mod_path);
        let mut edits = vec![JsEdit {
            start: start as u32,
            end: start as u32,
            text: {
                let mut text = frag!(import * as ~{&binding}~from ~{path};).print();
                text.push('\n');
                text
            },
        }];

        render_range_edits(
            db,
            macros,
            JsFileRange::new(db, file, start as u32, source.len() as u32),
            &binding,
            &mut edits,
        );

        edits
    }
}

fn render_range_edits<'db>(
    db: &'db dyn Db,
    macros: &[RsMacroSrc<'db>],
    range: JsFileRange<'db>,
    binding: &str,
    edits: &mut Vec<JsEdit>,
) {
    let mut cursor = range.start(db);

    for src in macros {
        let span = src.range(db);
        if span.start(db) < cursor || span.end(db) > range.end(db) {
            continue;
        }

        let diagnostics = src.check_compile_errors(db);
        if !diagnostics.is_empty() {
            let message = diagnostics
                .iter()
                .map(|diagnostic| {
                    diagnostic
                        .render(crate::diag::Theme::UnicodeNoColor)
                        .expect("rendering a diagnostic into a String cannot fail")
                })
                .collect::<Vec<_>>()
                .join("\n");
            let message = js_string(&message);
            edits.push(JsEdit {
                start: span.start(db),
                end: span.end(db),
                text: frag!({"(() => { throw new Error("}{message}{"); })()"}).print(),
            });
            cursor = span.end(db);
            continue;
        }

        let name = src.as_macro(db).symbol(db);
        let mut prefix = frag!({binding}{"."}{name}{"("}).print();
        let mut start = span.start(db);
        let mut has_arguments = false;

        for part in src.body(db) {
            if let JsPart::Interpolation { expression, .. } = part {
                edits.push(JsEdit {
                    start,
                    end: expression.start(db),
                    text: frag!({prefix}{"("}).print(),
                });
                render_range_edits(db, macros, *expression, binding, edits);
                start = expression.end(db);
                prefix = "), ".into();
                has_arguments = true;
            }
        }

        edits.push(JsEdit {
            start,
            end: span.end(db),
            text: if has_arguments {
                "))".into()
            } else {
                frag!({prefix}{")"}).print()
            },
        });
        cursor = span.end(db);
    }
}

fn js_string(value: &str) -> impl FragmentOnce + '_ {
    fragment(move |p| {
        frag!({ "\"" }).apply(p);

        for ch in value.chars() {
            match ch {
                '"' => frag!({ "\\\"" }).apply(p),
                '\\' => frag!({ "\\\\" }).apply(p),
                '\n' => frag!({ "\\n" }).apply(p),
                '\r' => frag!({ "\\r" }).apply(p),
                '\t' => frag!({ "\\t" }).apply(p),
                '\u{2028}' => frag!({ "\\u2028" }).apply(p),
                '\u{2029}' => frag!({ "\\u2029" }).apply(p),
                ch if ch < '\u{20}' => write!(p, "\\u{:04x}", ch as u32).unwrap(),
                ch => p.write_char(ch).unwrap(),
            }
        }

        frag!({ "\"" }).apply(p);
    })
}

#[cfg(test)]
mod tests {
    use crate::ir::js::JsFile;

    fn snapshot(name: &str, source: &str) {
        let db = crate::test_common::database();
        let file = JsFile::new(&db, "/input.tsx", source);
        let result = ra_ap_hir::attach_db(&db, || {
            file.module(&db)
                .render_transformed(&db, "rustier-next:runtime".into())
        });
        let output =
            format!("# Source\n\n```tsx\n{source}\n```\n\n# Result\n\n```tsx\n{result}\n```\n");
        insta::with_settings!({
            snapshot_path => "_snapshots/js_transform",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!(name, output);
        });
    }

    #[test]
    fn interpolation() {
        snapshot(
            "interpolation",
            "import { rs } from 'rustier';\nrs`${value}`;",
        );
    }

    #[test]
    fn aliases_and_shadowing() {
        snapshot(
            "aliases_and_shadowing",
            "import { rs as rust } from 'rustier';\nrust`${value}`;\nfunction shadow(rust) { return rust`not rust`; }",
        );
    }

    #[test]
    fn expressions_and_nesting() {
        snapshot(
            "expressions_and_nesting",
            "import { rs } from 'rustier';\nconst café = 1;\nrs`let _ = (${(café, other)}, ${condition ? `${café}` : other});`;\nrs`${rs`${value}`}`;",
        );
    }

    #[test]
    fn typescript_and_jsx() {
        snapshot(
            "typescript_and_jsx",
            "import { rs as rust } from 'rustier';\nimport type { rs } from 'rustier';\nconst x: number = 1;\nconst view = <div>{rust`${x}`}</div>;\nrs`not a value import`;",
        );
    }

    #[test]
    fn raw_template_text() {
        snapshot(
            "raw_template_text",
            "import { rs } from 'rustier';\nrs`__rustier_env.new_string(r#\"a\\nb\"#)`;\nconst text = `first\nsecond`;",
        );
    }

    #[test]
    fn no_macros() {
        snapshot(
            "no_macros",
            "import { rs } from 'another-library';\nrs`not a Rust macro`;",
        );
    }

    #[test]
    fn shebang_and_import_collision() {
        snapshot(
            "shebang_and_import_collision",
            "#!/usr/bin/env node\nimport { rs } from 'rustier';\nconst __rustier_ffi = 1;\nrs`${__rustier_ffi}`;",
        );
    }

    #[test]
    fn custom_ffi_module() {
        let db = crate::test_common::database();
        let source = "import { rs } from 'rustier';\nrs`${value}`;";
        let file = JsFile::new(&db, "/input.ts", source);
        let result = ra_ap_hir::attach_db(&db, || {
            file.module(&db)
                .render_transformed(&db, "./generated/ffi-macros.js".into())
        });
        insta::with_settings!({
            snapshot_path => "_snapshots/js_transform",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!("custom_ffi_module", format!("# Source\n\n```ts\n{source}\n```\n\n# Result\n\n```ts\n{result}\n```\n"));
        });
    }
}
