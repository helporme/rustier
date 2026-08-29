use std::sync::{Mutex, OnceLock};

use proptest::{
    prelude::*,
    test_runner::{Config, TestCaseError, basic_result_cache},
};

#[allow(dead_code)]
mod common;
#[path = "proptests/program.rs"]
mod program;
#[path = "proptests/strategy.rs"]
mod strategy;

use common::fragment::FragmentPrint as _;
use rust_curse::compiler::inputs::Platform;

static PROGRAM_COMPILER: OnceLock<Result<Mutex<common::ProgramCompiler>, String>> = OnceLock::new();

proptest! {
    #![proptest_config(Config {
        cases: 2048,
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
}

proptest! {
    #![proptest_config(Config {
        cases: 8,
        max_shrink_iters: 16,
        max_shrink_time: 10_000,
        result_cache: basic_result_cache,
        ..Config::default()
    })]

    #[test]
    fn generated_program_compiles_and_passes_cargo_check(
        program in strategy::program(),
    ) {
        let program = common::ProgramTest {
            src: program.print(),
            cargo_check: true,
            platforms: vec![Platform::Node],
        };

        let compiler = PROGRAM_COMPILER
            .get_or_init(|| common::ProgramCompiler::new().map(Mutex::new))
            .as_ref()
            .map_err(|error| TestCaseError::fail(error.clone()))?;
        let compiler = compiler.lock().map_err(|error| {
            TestCaseError::fail(format!("lock reusable program compiler: {error}"))
        })?;
        compiler.compile(&program).map_err(|error| {
            TestCaseError::fail(format!(
                "{error}\n\ngenerated source program:\n{}",
                program.src,
            ))
        })?;
    }
}
