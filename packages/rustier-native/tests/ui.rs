use std::{fs, io::ErrorKind, path::Path};

use itertools::Itertools;
use rustier::compiler::{codegen::shim::Platform, diag::Theme};

use crate::common::ProgramTest;

#[allow(dead_code)]
mod common;

struct UiTest {
    program: ProgramTest,
    diagnostic_codes: Vec<String>,
}

fn parse_ui_test(src: String) -> Result<UiTest, String> {
    let mut platforms = None;
    let mut cargo_check = false;
    let mut diagnostic_codes = Vec::new();

    for (line_index, line) in src.lines().enumerate() {
        let Some(directive) = line.trim_start().strip_prefix("//@") else {
            continue;
        };
        let directive = directive.trim();
        if let Some(value) = directive.strip_prefix("api:") {
            if platforms.is_some() {
                return Err(format!(
                    "duplicate `//@ api` directive on line {}",
                    line_index + 1
                ));
            }

            let mut parsed_platforms = Vec::new();
            for value in value.split(',').map(str::trim) {
                let platform = match value {
                    "wasm" | "wasm(web)" => Platform::Wasm,
                    "node" => Platform::Node,
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
                if parsed_platforms.contains(&platform) {
                    return Err(format!(
                        "duplicate API target `{value}` in `//@ api` directive on line {}",
                        line_index + 1
                    ));
                }
                parsed_platforms.push(platform);
            }
            platforms = Some(parsed_platforms);
        } else if directive == "cargo-check" {
            if cargo_check {
                return Err(format!(
                    "duplicate `//@ cargo-check` directive on line {}",
                    line_index + 1
                ));
            }
            cargo_check = true;
        } else if let Some(code) = directive.strip_prefix("diagnostic-code:") {
            let code = code.trim();
            if code.is_empty() {
                return Err(format!("empty diagnostic code on line {}", line_index + 1));
            }

            diagnostic_codes.push(code.to_owned());
        } else {
            return Err(format!(
                "unknown UI test directive `//@ {directive}` on line {}",
                line_index + 1
            ));
        }
    }

    let platforms = platforms.ok_or_else(|| {
        "missing `//@ api` directive; expected one or more of `wasm`, `node`, or `wasm(web)`"
            .to_string()
    })?;

    Ok(UiTest {
        program: ProgramTest {
            src,
            cargo_check,
            platforms,
        },
        diagnostic_codes,
    })
}

fn ui(fixture_path: &Path) -> datatest_stable::Result<()> {
    let source = fs::read_to_string(fixture_path).unwrap_or_else(|error| {
        panic!(
            "failed to read UI fixture {}: {error}",
            fixture_path.display()
        )
    });

    let test = parse_ui_test(source).unwrap_or_else(|error| {
        panic!(
            "failed to parse UI fixture {}: {error}",
            fixture_path.display()
        )
    });

    let diagnostics = test
        .program
        .compile()
        .unwrap_or_else(|error| panic!("UI fixture {} failed:\n{error}", fixture_path.display()));

    let mut actual = diagnostics
        .iter()
        .filter(|diagnostic| {
            if test.diagnostic_codes.is_empty() {
                return true;
            }
            let mut pending: Vec<&dyn miette::Diagnostic> = vec![diagnostic.inner.as_ref()];

            while let Some(diagnostic) = pending.pop() {
                if diagnostic
                    .code()
                    .is_some_and(|code| test.diagnostic_codes.contains(&code.to_string()))
                {
                    return true;
                }

                if let Some(related) = diagnostic.related() {
                    pending.extend(related);
                }
                if let Some(source) = diagnostic.diagnostic_source() {
                    pending.push(source);
                }
            }

            false
        })
        .map(|diagnostic| diagnostic.render(Theme::UnicodeNoColor))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| {
            panic!(
                "failed to render diagnostics for {}: {error}",
                fixture_path.display()
            )
        });
    actual.sort();

    let actual = actual.join("\n");

    let expected_path = fixture_path.with_extension("stderr");
    let expected = match fs::read_to_string(&expected_path) {
        Ok(expected) => expected,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => panic!(
            "failed to read expected UI output {}: {error}",
            expected_path.display()
        ),
    };
    let actual = actual.lines().map(str::trim_end).join("\n");
    let expected = expected.lines().map(str::trim_end).join("\n");

    if actual != expected {
        return Err(format!(
            "diagnostics differ for UI fixture {}\n\nexpected:\n{expected}\n\nactual:\n{actual}",
            fixture_path.display()
        )
        .into());
    }

    Ok(())
}

datatest_stable::harness! {
    { test = ui, root = "tests/ui", pattern = r".*\.rs$" },
}
