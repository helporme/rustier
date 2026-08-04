use std::{fs, io::ErrorKind, path::Path};

use rust_curse::compiler::{
    api::{ApiKind, WasmTarget},
    diag::Theme,
};

use crate::common::ProgramTest;

mod common;

fn parse_ui_test(src: String) -> Result<ProgramTest, String> {
    let mut apis = None;
    let mut cargo_check = false;

    for (line_index, line) in src.lines().enumerate() {
        let Some(directive) = line.trim_start().strip_prefix("//@") else {
            continue;
        };
        let directive = directive.trim();
        if let Some(value) = directive.strip_prefix("api:") {
            if apis.is_some() {
                return Err(format!(
                    "duplicate `//@ api` directive on line {}",
                    line_index + 1
                ));
            }

            let mut parsed_apis = Vec::new();
            for value in value.split(',').map(str::trim) {
                let api = match value {
                    "wasm" => ApiKind::Wasm {
                        target: WasmTarget::Node,
                    },
                    "node" => ApiKind::Node,
                    "wasm(web)" => ApiKind::Wasm {
                        target: WasmTarget::Web,
                    },
                    "" => {
                        return Err(format!(
                            "empty API target in `//@ api` directive on line {}",
                            line_index + 1
                        ));
                    }
                    _ => {
                        return Err(format!(
                            "unknown API target `{value}` in `//@ api` directive on line {}",
                            line_index + 1
                        ));
                    }
                };
                if parsed_apis.contains(&api) {
                    return Err(format!(
                        "duplicate API target `{value}` in `//@ api` directive on line {}",
                        line_index + 1
                    ));
                }
                parsed_apis.push(api);
            }
            apis = Some(parsed_apis);
        } else if directive == "cargo-check" {
            if cargo_check {
                return Err(format!(
                    "duplicate `//@ cargo-check` directive on line {}",
                    line_index + 1
                ));
            }
            cargo_check = true;
        } else {
            return Err(format!(
                "unknown UI test directive `//@ {directive}` on line {}",
                line_index + 1
            ));
        }
    }

    let apis = apis.ok_or_else(|| {
        "missing `//@ api` directive; expected one or more of `wasm`, `node`, or `wasm(web)`"
            .to_string()
    })?;

    Ok(ProgramTest {
        src,
        cargo_check,
        apis,
    })
}

fn ui(fixture_path: &Path) -> datatest_stable::Result<()> {
    let source = fs::read_to_string(fixture_path).unwrap_or_else(|error| {
        panic!(
            "failed to read UI fixture {}: {error}",
            fixture_path.display()
        )
    });

    let program = parse_ui_test(source).unwrap_or_else(|error| {
        panic!(
            "failed to parse UI fixture {}: {error}",
            fixture_path.display()
        )
    });

    let diagnostics = program
        .compile()
        .unwrap_or_else(|error| panic!("UI fixture {} failed:\n{error}", fixture_path.display()));

    let actual = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.render(Theme::UnicodeNoColor))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| {
            panic!(
                "failed to render diagnostics for {}: {error}",
                fixture_path.display()
            )
        })
        .join("\n");

    let expected_path = fixture_path.with_extension("stderr");
    let expected = match fs::read_to_string(&expected_path) {
        Ok(expected) => expected,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => panic!(
            "failed to read expected UI output {}: {error}",
            expected_path.display()
        ),
    };
    assert_eq!(
        actual,
        expected,
        "diagnostics differ for UI fixture {}",
        fixture_path.display()
    );

    Ok(())
}

datatest_stable::harness! {
    { test = ui, root = "tests/ui", pattern = r".*\.rs$" },
}
