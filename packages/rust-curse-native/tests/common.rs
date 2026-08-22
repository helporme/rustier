use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use rust_curse::{
    compiler::{
        codegen::api::{Api, ApiKind, WasmTarget},
        codegen::bindings::compile_bindings,
        db::RootDatabase,
        diag::Diag,
        rustdoc::{Crate, RustdocCrates},
    },
    sync::{SyncCrateOps, sync_crate},
    tools::rustdoc,
};
use serde_json::Value;
use tempfile::TempDir;

const SOURCE_CRATE_NAME: &str = "test_source";

/// A source program and the binding APIs against which it should be tested.
pub struct ProgramTest {
    /// Rust source code for the source crate.
    pub src: String,
    /// Whether generated crates should be checked by Cargo after syntax
    /// validation.
    pub cargo_check: bool,
    /// Binding APIs for which the source program should be compiled.
    pub apis: Vec<ApiKind>,
}

/// Reuses one filesystem workspace and Cargo target directory across program
/// tests.
pub struct ProgramCompiler {
    workspace: Workspace,
}

struct Workspace {
    _temp_dir: TempDir,
    source_dir: PathBuf,
    source_manifest: PathBuf,
    generated_root: PathBuf,
    cargo_target_dir: PathBuf,
}

impl Workspace {
    fn new() -> Result<Self, String> {
        let temp_dir = tempfile::Builder::new()
            .prefix("rust-curse-tests-")
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
            source_manifest,
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
        Workspace::new().map(|workspace| Self { workspace })
    }

    /// Compiles one program while retaining artifacts from earlier
    /// compilations.
    pub fn compile(&self, program: &ProgramTest) -> Result<Vec<Diag>, String> {
        if program.apis.is_empty() {
            return Err("program test must select at least one binding API".to_string());
        }

        let workspace = &self.workspace;
        fs::write(workspace.source_dir.join("src/lib.rs"), &program.src)
            .map_err(|error| format!("write source program: {error}"))?;

        let rustdoc_body = rustdoc::run(&workspace.source_manifest)
            .map_err(|error| format!("load source crate with rustdoc: {error}"))?;
        let crates = BTreeMap::from([(
            SOURCE_CRATE_NAME.to_string(),
            Crate {
                body: rustdoc_body,
                is_external: false,
                dir: workspace.source_dir.clone(),
            },
        )]);
        let mut db = RootDatabase::default();
        RustdocCrates::set(&mut db, crates);

        let mut diagnostics = Vec::new();
        for &api in &program.apis {
            let (api_name, generated_dir_name) = match api {
                ApiKind::Wasm {
                    target: WasmTarget::Node,
                } => ("wasm", "wasm"),
                ApiKind::Node => ("node", "node"),
                ApiKind::Wasm {
                    target: WasmTarget::Web,
                } => ("wasm(web)", "wasm-web"),
            };
            Api::set(&mut db, api);

            let generated_dir = workspace.generated_root.join(generated_dir_name);
            sync_crate(SyncCrateOps {
                manifest_path: &workspace.source_manifest,
                out_dir: &generated_dir,
                db: &db,
            })
            .map_err(|error| format!("compile and sync {api_name} bindings crate: {error}"))?;

            diagnostics.extend(
                compile_bindings::accumulated::<Diag>(&db)
                    .into_iter()
                    .cloned(),
            );

            let generated_source_path = generated_dir.join("src/lib.rs");
            let generated_source = fs::read_to_string(&generated_source_path)
                .map_err(|error| format!("read generated {api_name} bindings: {error}"))?;
            syn::parse_file(&generated_source).map_err(|error| {
                format!(
                    "generated {api_name} Rust did not parse at {}: {error}\n\n{generated_source}",
                    generated_source_path.display()
                )
            })?;

            if program.cargo_check {
                cargo_check_without_warnings(
                    &generated_dir.join("Cargo.toml"),
                    &workspace.cargo_target_dir,
                    &generated_source,
                )
                .map_err(|error| format!("{api_name} bindings failed cargo check\n{error}"))?;
            }
        }

        Ok(diagnostics)
    }
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
