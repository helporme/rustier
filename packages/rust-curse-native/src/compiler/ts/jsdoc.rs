use crate::compiler::fragment::{FragmentOnce, fragment};

/// Compiles optional Rust documentation into a TypeScript JSDoc comment.
pub fn compile_jsdoc(docs: Option<&str>) -> impl FragmentOnce + '_ {
    fragment(move |p| {
        let Some(docs) = docs else {
            return;
        };
        let trimmed = docs.trim();

        if trimmed.contains('\n') {
            "/**\n".apply(p);
            for line in trimmed.lines() {
                (" * ", line.trim(), "\n").apply(p);
            }
            " */\n".apply(p);
        } else {
            ("/** ", trimmed, " */\n").apply(p);
        }
    })
}
