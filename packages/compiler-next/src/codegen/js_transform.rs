use crate::{
    codegen::fragment::{FragmentOnce, FragmentPrint, doubles, frag, fragment},
    db::Db,
    ir::js::{JsFileRange, JsModule, JsPart, RsMacroSrc},
};

#[salsa::tracked]
impl<'db> JsModule<'db> {
    /// Replaces imported `rs` tags with calls to their exported snippet functions.
    ///
    /// Interpolations remain JS expressions and are evaluated at each call site,
    /// including nested tags. Untouched source is preserved verbatim; the helper
    /// import is inserted after any shebang with a collision-free local name.
    #[salsa::tracked(returns(ref))]
    pub fn render_transformed(self, db: &'db dyn Db, ffi_js_mod_path: String) -> String {
        let file = self.file(db);
        let macros = self.macros(db);
        let source = file.contents(db);

        if macros.is_empty() {
            return source.clone();
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
        let prefix = &source[..start];
        let body = render_range(
            db,
            macros,
            JsFileRange::new(db, file, start as u32, source.len() as u32),
            &binding,
        );
        let path = doubles(ffi_js_mod_path);
        frag!(
            {prefix}
            import * as ~{binding}~from ~{path};@
            {body}
        )
        .print()
    }
}

fn render_range<'db>(
    db: &'db dyn Db,
    macros: &[RsMacroSrc<'db>],
    range: JsFileRange<'db>,
    binding: &str,
) -> String {
    let source = range.file(db).contents(db);

    fragment(|printer| {
        let mut cursor = range.start(db);

        for src in macros {
            let span = src.range(db);
            if span.start(db) < cursor || span.end(db) > range.end(db) {
                continue;
            }

            source[cursor as usize..span.start(db) as usize].apply(printer);

            let name = src.as_macro(db).symbol(db);
            let arguments = fragment(|printer| {
                let mut first = true;

                for part in src.body(db) {
                    if let JsPart::Interpolation { expression, .. } = part {
                        if !first {
                            frag!(, ~).apply(printer);
                        }
                        first = false;

                        let argument = render_range(db, macros, *expression, binding);
                        frag!({"("}{argument}{")"}).apply(printer);
                    }
                }
            });

            frag!({binding}{"."}{name}{"("}{arguments}{")"}).apply(printer);
            cursor = span.end(db);
        }

        source[cursor as usize..range.end(db) as usize].apply(printer);
    })
    .print()
}

#[cfg(test)]
mod tests {
    use crate::{db::RootDatabase, ir::js::JsFile};

    fn snapshot(name: &str, source: &str) {
        let db = RootDatabase::default();
        let file = JsFile::new(&db, "/input.tsx", source);
        let result = file
            .module(&db)
            .render_transformed(&db, "rustier-next:runtime".into());
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
            "import { rs } from 'rustier';\nconst café = 1;\nrs`(${(café, other)}, ${condition ? `${café}` : other})`;\nrs`consume(${rs`${value}`})`;",
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
            "import { rs } from 'another-library';\nrs`not a Rust snippet`;",
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
        let db = RootDatabase::default();
        let source = "import { rs } from 'rustier';\nrs`${value}`;";
        let file = JsFile::new(&db, "/input.ts", source);
        let result = file
            .module(&db)
            .render_transformed(&db, "./generated/ffi-snippets.js".into());
        insta::with_settings!({
            snapshot_path => "_snapshots/js_transform",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!("custom_ffi_module", format!("# Source\n\n```ts\n{source}\n```\n\n# Result\n\n```ts\n{result}\n```\n"));
        });
    }
}
