use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

use rust_curse::{
    codegen::{
        api::{Api, ApiKind},
        db::DbImpl,
        rustdoc::{Crate, RustdocCrates},
    },
    sync::{SyncCrateOps, sync_crate},
    tools::rustdoc,
};
use serde_json::Value;
use tempfile::TempDir;

const SOURCE_CRATE_NAME: &str = "proptest_source";

static WORKSPACE: OnceLock<Result<Workspace, String>> = OnceLock::new();

struct Workspace {
    _temp_dir: TempDir,
    source_dir: PathBuf,
    source_manifest: PathBuf,
    generated_dir: PathBuf,
    cargo_target_dir: PathBuf,
}

impl Workspace {
    fn new() -> Result<Self, String> {
        let temp_dir = tempfile::Builder::new()
            .prefix("rust-curse-proptests-")
            .tempdir()
            .map_err(|error| format!("create persistent proptest workspace: {error}"))?;
        let source_dir = temp_dir.path().join("source");
        let generated_dir = temp_dir.path().join("generated");
        let cargo_target_dir = temp_dir.path().join("target");
        let source_manifest = source_dir.join("Cargo.toml");

        fs::create_dir_all(source_dir.join("src"))
            .map_err(|error| format!("create source crate: {error}"))?;
        fs::write(
            &source_manifest,
            r#"[package]
name = "proptest-source"
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
            generated_dir,
            cargo_target_dir,
        })
    }
}

/// Compiles one generated source program through the full Node bindings
/// pipeline.
pub fn compile_program(source: &str) -> Result<(), String> {
    let workspace = WORKSPACE
        .get_or_init(Workspace::new)
        .as_ref()
        .map_err(Clone::clone)?;
    fs::write(workspace.source_dir.join("src/lib.rs"), source)
        .map_err(|error| format!("write generated source program: {error}"))?;

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
    let mut db = DbImpl::default();
    RustdocCrates::set(&mut db, crates);
    Api::set(&mut db, ApiKind::Node);

    sync_crate(SyncCrateOps {
        manifest_path: &workspace.source_manifest,
        out_dir: &workspace.generated_dir,
        db: &db,
    })
    .map_err(|error| format!("compile and sync generated bindings crate: {error}"))?;

    let generated_source_path = workspace.generated_dir.join("src/lib.rs");
    let generated_source = fs::read_to_string(&generated_source_path)
        .map_err(|error| format!("read generated bindings: {error}"))?;
    syn::parse_file(&generated_source).map_err(|error| {
        format!(
            "generated Rust did not parse at {}: {error}\n\n{generated_source}",
            generated_source_path.display()
        )
    })?;

    cargo_check_without_warnings(
        &workspace.generated_dir.join("Cargo.toml"),
        &workspace.cargo_target_dir,
        &generated_source,
    )
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
