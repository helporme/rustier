use std::fmt::Write as _;

use crate::{
    codegen::fragment::{FragmentOnce, FragmentPrint, doubles, frag, fragment},
    db::Db,
    ir::js::{JsFileRange, JsModule, JsPart, RsMacroSrc},
    ir::runtime::{RsPart, Runtime},
};

#[derive(Clone, PartialEq, Eq, salsa::Update)]
pub struct JsEdit {
    pub start: u32,
    pub end: u32,
    pub text: String,
}

pub fn edits_to_utf16<'a>(
    source: &'a str,
    edits: &'a [JsEdit],
) -> impl Iterator<Item = JsEdit> + 'a {
    let mut byte_offset = 0;
    let mut utf16_offset = 0;

    edits.iter().map(move |edit| {
        utf16_offset += source[byte_offset..edit.start as usize]
            .encode_utf16()
            .count() as u32;
        let start = utf16_offset;
        utf16_offset += source[edit.start as usize..edit.end as usize]
            .encode_utf16()
            .count() as u32;
        byte_offset = edit.end as usize;

        JsEdit {
            start,
            end: utf16_offset,
            text: edit.text.clone(),
        }
    })
}

#[salsa::tracked]
impl<'db> Runtime<'db> {
    /// Replaces imported `rs` tags with calls to their exported macro functions.
    #[salsa::tracked(returns(ref))]
    pub fn render_transformed(
        self,
        db: &'db dyn Db,
        module: JsModule<'db>,
        ffi_js_mod_path: String,
    ) -> String {
        let source = module.file(db).contents(db);

        fragment(|p| {
            let mut cursor = 0;

            for edit in self.render_transform_edits(db, module, ffi_js_mod_path) {
                source[cursor..edit.start as usize].apply(p);
                edit.text.as_str().apply(p);
                cursor = edit.end as usize;
            }

            source[cursor..].apply(p);
        })
        .print()
    }

    #[salsa::tracked(returns(ref))]
    pub fn render_transform_edits(
        self,
        db: &'db dyn Db,
        module: JsModule<'db>,
        ffi_js_mod_path: String,
    ) -> Vec<JsEdit> {
        module.transform_edits(db, ffi_js_mod_path, Some(self))
    }
}

impl<'db> JsModule<'db> {
    pub fn syntax_transform_edits(self, db: &'db dyn Db, runtime_path: String) -> Vec<JsEdit> {
        self.transform_edits(db, runtime_path, None)
    }

    fn transform_edits(
        self,
        db: &'db dyn Db,
        ffi_js_mod_path: String,
        runtime: Option<Runtime<'db>>,
    ) -> Vec<JsEdit> {
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
            runtime,
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
    runtime: Option<Runtime<'db>>,
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

        if let Some(runtime) = runtime
            && let diagnostics = runtime.source_diagnostics(db, *src)
            && !diagnostics.is_empty()
        {
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

        let name = RsPart::symbol(&src.rs_body(db));
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
                render_range_edits(db, runtime, macros, *expression, binding, edits);
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
    use crate::ir::{
        js::{JsFile, JsProgram},
        runtime::Runtime,
    };

    fn snapshot(name: &str, source: &str) {
        let (db, rs) = crate::test_common::database();
        let file = JsFile::new(&db, "/input.tsx", source);
        let result = ra_ap_hir::attach_db(&db, || {
            Runtime::new(&db, JsProgram::new(&db, vec![file]), rs).render_transformed(
                &db,
                file.module(&db),
                "rustier:runtime".into(),
            )
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
            "import { rs } from 'rustier';\nrs`let _ = r#\"a\\nb\"#;`;\nconst text = `first\nsecond`;",
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
        let (db, rs) = crate::test_common::database();
        let source = "import { rs } from 'rustier';\nrs`${value}`;";
        let file = JsFile::new(&db, "/input.ts", source);
        let result = ra_ap_hir::attach_db(&db, || {
            Runtime::new(&db, JsProgram::new(&db, vec![file]), rs).render_transformed(
                &db,
                file.module(&db),
                "./generated/ffi-macros.js".into(),
            )
        });
        insta::with_settings!({
            snapshot_path => "_snapshots/js_transform",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!("custom_ffi_module", format!("# Source\n\n```ts\n{source}\n```\n\n# Result\n\n```ts\n{result}\n```\n"));
        });
    }
}
