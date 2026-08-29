use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use rust_curse::compiler::{
    CompileShimOpts, Compiler, CompilerDiagnostic, CrateApiDiscovery, CrateApiImportMode,
    CrateGraphBuilder, Edition, FilePath, Platform, WorkspaceChange, WorkspaceCrate,
    WorkspaceCrateKind,
};
use serde_json::Value;
use tempfile::TempDir;

#[path = "common/fragment.rs"]
pub mod fragment;

const SOURCE_CRATE_NAME: &str = "test_source";
const COMPILER_STD_FIXTURE: &str = r#"
mod __rust_curse_test_std {
    pub mod string {
        pub struct String;
    }
}
use __rust_curse_test_std::string::String;
"#;

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
    workspace: Workspace,
}

struct Workspace {
    _temp_dir: TempDir,
    source_dir: PathBuf,
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
            generated_root,
            cargo_target_dir,
        })
    }
}

impl ProgramTest {
    /// Compiles this program for every selected API and returns emitted
    /// diagnostics.
    pub fn compile(&self) -> Result<Vec<CompilerDiagnostic>, String> {
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
    pub fn compile(&self, program: &ProgramTest) -> Result<Vec<CompilerDiagnostic>, String> {
        if program.platforms.is_empty() {
            return Err("program test must select at least one platform".to_string());
        }

        let workspace = &self.workspace;
        fs::write(workspace.source_dir.join("src/lib.rs"), &program.src)
            .map_err(|error| format!("write source program: {error}"))?;

        let mut diagnostics = Vec::new();
        for &platform in &program.platforms {
            let (platform_name, generated_dir_name) = match platform {
                Platform::Wasm {
                    target: rust_curse::compiler::WasmEnv::Node,
                } => ("wasm", "wasm"),
                Platform::Node => ("node", "node"),
                Platform::Wasm {
                    target: rust_curse::compiler::WasmEnv::Web,
                } => ("wasm(web)", "wasm-web"),
                _ => return Err("program test selected an unsupported platform".to_string()),
            };
            let generated_dir = workspace.generated_root.join(generated_dir_name);
            let root_file = real_file_path(&workspace.source_dir.join("src/lib.rs"))?;
            let mut compiler = Compiler::new();
            let mut change = WorkspaceChange::new();
            change.set_local_file(
                root_file.clone(),
                Some(format!("{}\n{COMPILER_STD_FIXTURE}", program.src)),
            );
            let mut crates = CrateGraphBuilder::default();
            crates.add_crate(WorkspaceCrate {
                name: SOURCE_CRATE_NAME.to_owned(),
                edition: Edition::E2024,
                root_file,
                version: "0.0.0".to_owned(),
                kind: WorkspaceCrateKind::Local,
                api_discovery: CrateApiDiscovery::Eager,
                api_import_mode: CrateApiImportMode::Mod,
            });
            change.set_crates(crates);
            compiler.change(change).map_err(|error| {
                format!("configure {platform_name} compiler workspace: {error}")
            })?;

            let output = compiler.compile_shim(CompileShimOpts { platform });
            diagnostics.extend(output.diagnostics);
            write_generated_crate(
                &generated_dir,
                &workspace.source_dir,
                platform,
                &output.shim,
            )?;

            let generated_source_path = generated_dir.join("src/lib.rs");
            let generated_source = fs::read_to_string(&generated_source_path)
                .map_err(|error| format!("read generated {platform_name} bindings: {error}"))?;
            syn::parse_file(&generated_source).map_err(|error| {
                format!(
                    "generated {platform_name} Rust did not parse at {}: {error}\n\n{generated_source}",
                    generated_source_path.display()
                )
            })?;

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

fn real_file_path(path: &Path) -> Result<FilePath, String> {
    FilePath::new_real(path.to_string_lossy())
        .map_err(|error| format!("create compiler path for {}: {error}", path.display()))
}

fn write_generated_crate(
    generated_dir: &Path,
    source_dir: &Path,
    platform: Platform,
    shim: &str,
) -> Result<(), String> {
    fs::create_dir_all(generated_dir.join("src"))
        .map_err(|error| format!("create generated crate directory: {error}"))?;
    fs::write(generated_dir.join("src/lib.rs"), shim)
        .map_err(|error| format!("write generated shim: {error}"))?;

    let source_dir = source_dir.to_string_lossy().replace('\\', "\\\\");
    let (dependencies, build) = match platform {
        Platform::Node => (
            "napi = { version = \"3\", features = [\"napi10\"] }\nnapi-derive = \"3\"\n\n[build-dependencies]\nnapi-build = \"2\"\n",
            "build = \"build.rs\"\n",
        ),
        Platform::Wasm { .. } => ("wasm-bindgen = \"0.2\"\n", ""),
        _ => unreachable!("program tests only select supported platforms"),
    };
    let manifest = format!(
        r#"[package]
name = "generated-test-source"
version = "0.0.0"
edition = "2024"
publish = false
{build}
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
test_source = {{ package = "test-source", path = "{source_dir}" }}
{dependencies}"#
    );
    fs::write(generated_dir.join("Cargo.toml"), manifest)
        .map_err(|error| format!("write generated manifest: {error}"))?;

    if platform == Platform::Node {
        fs::write(
            generated_dir.join("build.rs"),
            "fn main() { napi_build::setup(); }\n",
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
