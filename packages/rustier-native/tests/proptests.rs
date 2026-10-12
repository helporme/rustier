use std::sync::{Mutex, OnceLock};

use proptest::{
    prelude::*,
    test_runner::{
        Config, FileFailurePersistence, TestCaseError, TestCaseResult, basic_result_cache,
    },
};

#[allow(dead_code)]
mod common;
#[path = "proptests/program.rs"]
mod program;
#[path = "proptests/strategy.rs"]
mod strategy;

use common::fragment::FragmentPrint as _;
use program::Program;
use rustier::compiler::codegen::runtime::Platform;

const REGRESSION_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/proptests.proptest-regressions"
);
static PROGRAM_COMPILER: OnceLock<Result<Mutex<common::ProgramCompiler>, String>> = OnceLock::new();

fn compile_generated_program(program: Program, platforms: Vec<Platform>) -> TestCaseResult {
    let exported_functions = program.exported_functions();
    let program = common::ProgramTest {
        src: program.print(),
        cargo_check: true,
        platforms,
    };
    let compiler = PROGRAM_COMPILER
        .get_or_init(|| common::ProgramCompiler::new().map(Mutex::new))
        .as_ref()
        .map_err(|error| TestCaseError::fail(error.clone()))?;
    let compiler = compiler
        .lock()
        .map_err(|error| TestCaseError::fail(format!("lock reusable program compiler: {error}")))?;

    compiler
        .compile_with_exports(&program, &exported_functions)
        .map_err(|error| {
            TestCaseError::fail(format!(
                "{error}\n\ngenerated source program:\n{}",
                program.src,
            ))
        })?;

    Ok(())
}

proptest! {
    #![proptest_config(Config {
        cases: 2048,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_FILE))),
        ..Config::default()
    })]

    #[test]
    fn generated_program_is_valid_rust_syntax(program in strategy::program()) {
        let source = program.print();
        syn::parse_file(&source).map_err(|error| {
            TestCaseError::fail(format!(
                "generated source did not parse: {error}\n\ngenerated source program:\n{source}"
            ))
        })?;
    }

    #[test]
    fn generated_generic_program_is_valid_rust_syntax(program in strategy::generic_program()) {
        let source = program.print();
        syn::parse_file(&source).map_err(|error| {
            TestCaseError::fail(format!(
                "generated source did not parse: {error}\n\ngenerated source program:\n{source}"
            ))
        })?;
    }
}

proptest! {
    #![proptest_config(Config {
        cases: 8,
        max_shrink_iters: 16,
        max_shrink_time: 10_000,
        result_cache: basic_result_cache,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(REGRESSION_FILE))),
        ..Config::default()
    })]

    #[test]
    fn generated_program_compiles_and_passes_cargo_check(
        program in strategy::program(),
    ) {
        compile_generated_program(program, vec![Platform::Node])?;
    }

    #[test]
    fn generated_generic_program_compiles_and_passes_cargo_check(
        program in strategy::generic_program(),
    ) {
        compile_generated_program(program, vec![Platform::Node, Platform::Wasm])?;
    }
}

#[test]
fn omitted_rust_exports_are_rejected() -> Result<(), String> {
    let compiler = common::ProgramCompiler::new()?;
    let cases = [
        (
            frag!(
                pub fn generic<T>(value: T) -> T {
                    { value }
                }
            )
            .print(),
            vec!["generic".to_owned()],
        ),
        (
            frag!(
                pub fn present() {
                    {}
                }
            )
            .print(),
            vec!["missing".to_owned(), "present".to_owned()],
        ),
    ];

    for (src, export) in cases {
        let program = common::ProgramTest {
            src,
            cargo_check: false,
            platforms: vec![Platform::Node],
        };

        match compiler.compile_with_exports(&program, &[export]) {
            Err(error) if error.contains("bindings omitted expected exported function") => {}
            Err(error) => {
                return Err(format!(
                    "expected a missing Rust export error, got:\n{error}"
                ));
            }
            Ok(_) => {
                return Err("missing Rust export was silently accepted".to_owned());
            }
        }
    }

    Ok(())
}
