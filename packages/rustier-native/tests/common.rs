use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace_at};
use ra_ap_project_model::{CargoConfig, RustLibSource};
use rustier::compiler::{
    codegen::runtime::{Platform, manifest::Manifest},
    diag::Diag,
    inputs::LocalCrates,
    ir::rust::{PathSegment, Rust, usages::UsageGraph},
};
use serde_json::Value;
use tempfile::TempDir;

#[path = "common/fragment.rs"]
pub mod fragment;

const SOURCE_CRATE_NAME: &str = "test_source";

/// A source program and the binding APIs against which it should be tested.
pub struct ProgramTest {
    /// Rust source code for the source crate.
    pub src: String,
    /// Whether generated crates should be checked by Cargo after syntax
    /// validation.
    pub cargo_check: bool,
    /// Platforms for which the source program should be compiled.
    pub platforms: Vec<Platform>,
}

/// Reuses one filesystem workspace and Cargo target directory across program
/// tests.
pub struct ProgramCompiler {
    workspace: TestWorkspace,
}

struct TestWorkspace {
    _temp_dir: TempDir,
    source_dir: PathBuf,
    generated_root: PathBuf,
    cargo_target_dir: PathBuf,
}

impl TestWorkspace {
    fn new() -> Result<Self, String> {
        let temp_dir = tempfile::Builder::new()
            .prefix("rustier-tests-")
            .tempdir()
            .map_err(|error| format!("create isolated test workspace: {error}"))?;
        let source_dir = temp_dir.path().join("source");
        let generated_root = temp_dir.path().join("generated");
        let cargo_target_dir = temp_dir.path().join("target");
        let source_manifest = source_dir.join("Cargo.toml");

        fs::create_dir_all(source_dir.join("src"))
            .map_err(|error| format!("create source crate: {error}"))?;
        fs::write(
            &source_manifest,
            r#"[package]
name = "test-source"
version = "0.0.0"
edition = "2024"
publish = false

[lib]
path = "src/lib.rs"

[workspace]
"#,
        )
        .map_err(|error| format!("write source manifest: {error}"))?;

        Ok(Self {
            _temp_dir: temp_dir,
            source_dir,
            generated_root,
            cargo_target_dir,
        })
    }
}

impl ProgramTest {
    /// Compiles this program for every selected API and returns emitted
    /// diagnostics.
    pub fn compile(&self) -> Result<Vec<Diag>, String> {
        ProgramCompiler::new()?.compile(self)
    }
}

impl ProgramCompiler {
    /// Creates an isolated compiler workspace whose build artifacts can be
    /// reused.
    pub fn new() -> Result<Self, String> {
        TestWorkspace::new().map(|workspace| Self { workspace })
    }

    /// Compiles one program while retaining artifacts from earlier
    /// compilations.
    pub fn compile(&self, program: &ProgramTest) -> Result<Vec<Diag>, String> {
        self.compile_with_exports(program, &[])
    }

    pub fn compile_with_exports(
        &self,
        program: &ProgramTest,
        exported_functions: &[Vec<String>],
    ) -> Result<Vec<Diag>, String> {
        if program.platforms.is_empty() {
            return Err("program test must select at least one platform".to_string());
        }

        if exported_functions.iter().any(Vec::is_empty) {
            return Err("expected function path must not be empty".to_owned());
        }

        let workspace = &self.workspace;
        fs::write(workspace.source_dir.join("src/lib.rs"), &program.src)
            .map_err(|error| format!("write source program: {error}"))?;

        let mut diagnostics = Vec::new();
        for (platform_index, &platform) in program.platforms.iter().enumerate() {
            let (platform_name, generated_dir_name) = match platform {
                Platform::Wasm => ("wasm", "wasm"),
                Platform::Node => ("node", "node"),
                _ => return Err("program test selected an unsupported platform".to_string()),
            };
            let generated_dir = workspace.generated_root.join(generated_dir_name);
            let (mut db, _, _) = load_workspace_at(
                &workspace.source_dir,
                &CargoConfig {
                    sysroot: Some(RustLibSource::Discover),
                    ..CargoConfig::default()
                },
                &LoadCargoConfig {
                    load_out_dirs_from_check: false,
                    with_proc_macro_server: ProcMacroServerChoice::None,
                    prefill_caches: false,
                    num_worker_threads: 1,
                    proc_macro_processes: 1,
                },
                &|_| {},
            )
            .map_err(|error| format!("configure {platform_name} compiler workspace: {error}"))?;
            let root_file = ra_ap_hir::attach_db(&db, || {
                Rust::get(&db)
                    .crates(&db)
                    .iter()
                    .find(|crate_| crate_.name(&db) == SOURCE_CRATE_NAME)
                    .expect("source crate should be lowered")
                    .root(&db)
                    .file_path(&db)
                    .clone()
            });

            LocalCrates::set(&mut db, [root_file]);

            let db = &db;
            let (interface, expected_symbols) = ra_ap_hir::attach_db(db, || {
                let rust = Rust::get(db);
                let source = rust
                    .crates(db)
                    .iter()
                    .find(|crate_| crate_.name(db) == SOURCE_CRATE_NAME)
                    .copied()
                    .expect("source crate should be lowered");

                let interface =
                    rust.interface_reachable_from(db, UsageGraph::new(db, vec![source.root(db)]));

                if platform_index == 0 {
                    diagnostics.extend(interface.check(db).into_iter().cloned());
                }

                let expected_symbols = exported_functions
                    .iter()
                    .map(|names| {
                        let path = names.iter().fold(source.root(db).path(db), |path, name| {
                            path.append(db, PathSegment::plain(db, name.clone()))
                        });

                        format!("__fn__free__{}", path.mangled(db))
                    })
                    .collect::<Vec<_>>();

                (interface.render(db, platform).to_owned(), expected_symbols)
            });

            write_generated_crate(&generated_dir, &workspace.source_dir, platform, &interface)?;

            let generated_source_path = generated_dir.join("src/lib.rs");
            let generated_source = fs::read_to_string(&generated_source_path)
                .map_err(|error| format!("read generated {platform_name} bindings: {error}"))?;
            let parsed = syn::parse_file(&generated_source).map_err(|error| {
                format!(
                    "generated {platform_name} Rust did not parse at {}: {error}\n\n{generated_source}",
                    generated_source_path.display()
                )
            })?;
            let emitted_functions = parsed
                .items
                .iter()
                .filter_map(|item| match item {
                    syn::Item::Fn(function) => Some(function.sig.ident.to_string()),
                    _ => None,
                })
                .collect::<HashSet<_>>();

            for symbol in expected_symbols {
                if !emitted_functions.contains(&symbol) {
                    return Err(format!(
                        "{platform_name} bindings omitted expected exported function `{symbol}`\n\n{generated_source}"
                    ));
                }
            }

            if program.cargo_check {
                cargo_check_without_warnings(
                    &generated_dir.join("Cargo.toml"),
                    &workspace.cargo_target_dir,
                    &generated_source,
                )
                .map_err(|error| format!("{platform_name} bindings failed cargo check\n{error}"))?;
            }
        }

        Ok(diagnostics)
    }
}

fn write_generated_crate(
    generated_dir: &Path,
    source_dir: &Path,
    platform: Platform,
    interface: &str,
) -> Result<(), String> {
    fs::create_dir_all(generated_dir.join("src"))
        .map_err(|error| format!("create generated crate directory: {error}"))?;
    fs::write(generated_dir.join("src/lib.rs"), interface)
        .map_err(|error| format!("write generated runtime: {error}"))?;

    let manifest = Manifest {
        name: "test-source",
        source_package_name: "test-source",
        source_crate_name: SOURCE_CRATE_NAME,
        source_package_path: source_dir,
        runtime_path: Some(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../rustier-rt"
        ))),
        platform: platform.into(),
    };
    fs::write(generated_dir.join("Cargo.toml"), manifest.render())
        .map_err(|error| format!("write generated manifest: {error}"))?;

    if let Some(body) = manifest.platform.build_script_body() {
        fs::write(
            generated_dir.join("build.rs"),
            format!("fn main() {{ {body} }}\n"),
        )
        .map_err(|error| format!("write generated build script: {error}"))?;
    }

    Ok(())
}

fn cargo_check_without_warnings(
    manifest_path: &Path,
    target_dir: &Path,
    generated_source: &str,
) -> Result<(), String> {
    let output = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(manifest_path)
        .arg("--target-dir")
        .arg(target_dir)
        .arg("--message-format=json")
        .output()
        .map_err(|error| format!("run cargo check on generated bindings crate: {error}"))?;

    let mut compiler_diagnostics = Vec::new();
    let mut warnings = Vec::new();
    for line in output.stdout.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        let message: Value = serde_json::from_slice(line)
            .map_err(|error| format!("parse cargo JSON message: {error}"))?;
        if message["reason"] == "compiler-message" {
            let diagnostic = message["message"]["rendered"]
                .as_str()
                .or_else(|| message["message"]["message"].as_str())
                .unwrap_or("compiler emitted a diagnostic")
                .trim()
                .to_string();
            if message["message"]["level"] == "warning" {
                warnings.push(diagnostic.clone());
            }
            compiler_diagnostics.push(diagnostic);
        }
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    warnings.extend(
        stderr
            .lines()
            .filter(|line| line.trim_start().starts_with("warning:"))
            .map(str::to_string),
    );

    if !output.status.success() || !warnings.is_empty() {
        return Err(format!(
            "cargo check failed or emitted warnings (status: {})\nwarnings:\n{}\ncompiler diagnostics:\n{}\nstderr:\n{}\n\ngenerated bindings:\n{}",
            output.status,
            if warnings.is_empty() {
                "<none>".to_string()
            } else {
                warnings.join("\n")
            },
            if compiler_diagnostics.is_empty() {
                "<none>".to_string()
            } else {
                compiler_diagnostics.join("\n")
            },
            stderr.trim(),
            generated_source,
        ));
    }

    Ok(())
}
