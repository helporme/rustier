use crate::{
    db::RootDatabase,
    ir::{JsFile, Part, Transport},
};

fn snapshot(name: &str, source: &str) {
    let db = RootDatabase::default();
    let file = JsFile::new(&db, "/input.tsx".to_owned(), source.to_owned());
    let mut output = format!("# Source\n\n```tsx\n{}\n```\n\n", source.trim_end());

    match file.macros(&db) {
        Ok(macros) => {
            output.push_str("# Macros\n\n");
            for macro_ in macros {
                let range = macro_.range(&db);
                output.push_str(&format!(
                    "{}: {}..{}\n",
                    macro_.name(&db),
                    range.start,
                    range.end
                ));
                let mut index = 0;
                for part in macro_.body(&db) {
                    if let Part::Interpolation {
                        type_,
                        range,
                        expression,
                    } = part
                    {
                        output.push_str(&format!(
                            "  {index}: {} at {}..{}, expression {}..{}: {:?}\n",
                            type_.source(&db),
                            range.start,
                            range.end,
                            expression.start,
                            expression.end,
                            &source[expression.start as usize..expression.end as usize]
                        ));
                        index += 1;
                    }
                }
            }
            let transport = Transport::new(&db, macros.clone());
            let rust = transport.render(&db);
            let syntax = syn::parse_file(rust)
                .unwrap_or_else(|error| panic!("invalid generated Rust: {error}\n{rust}"));
            output.push_str(&format!(
                "\n# Wasm\n\n```rust\n{}```\n",
                prettyplease::unparse(&syntax)
            ));
        }
        Err(diagnostics) => {
            output.push_str("# Diagnostics\n\n");
            for diagnostic in diagnostics {
                output.push_str(&format!("{diagnostic:?}\n"));
            }
        }
    }

    insta::with_settings!({
        snapshot_path => "_snapshots",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_snapshot!(name, output);
    });
}

macro_rules! snapshot_test {
    ($name:ident, $source:literal) => {
        #[test]
        fn $name() {
            snapshot(stringify!($name), $source);
        }
    };
}

snapshot_test!(
    interpolation,
    r#"
    import { rs } from "rustier";

    let x = 0;
    rs`
      let x = ${x};
    `
    "#
);

snapshot_test!(
    constant,
    r#"
    import { rs } from "rustier";

    rs`
      10
    `
    "#
);

snapshot_test!(
    aliases_and_shadowing,
    r#"
import { rs as rust } from "rustier";
import { rs } from "another-library";
rust`${1}`;
rs`not rust`;
function shadow(rust) {
    rust`not rust either`;
}
{
    const rust = String.raw;
    rust`also not rust`;
}
rust`done`;
"#
);

snapshot_test!(
    expressions_and_nesting,
    r#"
import { rs } from "rustier";
const café = 1;
rs`(${ /* keep */ (café + 1) }, ${condition ? `${café}` : other})`;
rs`consume(${rs`make_value()`})`;
"#
);

snapshot_test!(
    typescript_and_jsx,
    r#"
import { rs as rust } from "rustier";
import type { rs } from "rustier";
const x: number = 1;
const view = <div>{rust`${x}`}</div>;
rs`not a value import`;
"#
);

snapshot_test!(
    raw_template_text,
    r##"
import { rs } from "rustier";
rs`r#"a\nb"#`;
rs`"\u{1f600}"`;
"##
);

snapshot_test!(
    no_macros,
    r#"
import { rs } from "another-library";
const text = "rs`not a macro`";
rs`also not a macro`;
"#
);

snapshot_test!(
    invalid_javascript,
    r#"
import { rs } from "rustier";
rs`${`;
"#
);
