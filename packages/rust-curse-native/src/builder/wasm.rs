//! Wasm build driver: provisions `wasm32-unknown-unknown` + `wasm-bindgen-cli`
//! and turns a wasm-feature-enabled crate into JS/TS glue + a `.wasm` artifact.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

use crate::builder::{self, BuildError, BuildOpts};
use crate::sync::write_wasm::WASM_CARGO_FEATURE;

const WASM_TARGET_TRIPLE: &str = "wasm32-unknown-unknown";

#[derive(Debug, Error)]
pub enum WasmBuildError {
    #[error("rustup is not available")]
    RustupNotAvailable(#[source] io::Error),

    #[error("rustup target list failed (exit code {exit_code:?}):\n{stderr}")]
    RustupTargetListFailed {
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("rustup target add failed (exit code {exit_code:?}):\n{stderr}")]
    RustupTargetAddFailed {
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("cargo is not available")]
    CargoNotAvailable(#[source] io::Error),

    #[error("cargo install wasm-bindgen-cli failed (exit code {exit_code:?}):\n{stderr}")]
    WasmBindgenInstallFailed {
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("io error at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error(transparent)]
    Build(#[from] BuildError),

    #[error("wasm-bindgen failed (exit code {exit_code:?}):\n{stderr}")]
    WasmBindgenFailed {
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("compiled .wasm not found at '{}'", .0.display())]
    WasmArtifactMissing(PathBuf),

    #[error("cargo metadata failed (exit code {exit_code:?}):\n{stderr}")]
    MetadataFailed {
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("failed to parse cargo metadata output")]
    MetadataParseFailed(#[source] serde_json::Error),

    #[error("no library target found in cargo metadata")]
    NoLibTarget,
}

pub struct BuildWasmOpts<'a> {
    /// Project containing the wasm-feature-enabled Cargo.toml.
    pub project_root_abs_path: &'a Path,
    /// Where wasm-bindgen-cli was installed (the same dir passed to
    /// [`install_wasm_tools`]).
    pub tools_dir_abs_path: &'a Path,
    /// Where to write `target/` and `package/`. Typically
    /// `<project>/.rust-curse/output/wasm`.
    pub wasm_output_dir_abs_path: &'a Path,
    pub release: bool,
}

/// Ensure `wasm32-unknown-unknown` is installed via rustup and
/// `wasm-bindgen-cli` is installed under `<tools_dir>/wasm-bindgen-cli`.
/// Idempotent: skips work that's already done.
pub fn install_wasm_tools(tools_dir_abs_path: &Path) -> Result<(), WasmBuildError> {
    ensure_wasm_target()?;
    ensure_wasm_bindgen_cli(tools_dir_abs_path)?;
    Ok(())
}

pub fn build_wasm(opts: BuildWasmOpts<'_>) -> Result<(), WasmBuildError> {
    install_wasm_tools(opts.tools_dir_abs_path)?;

    let lib_name = read_lib_name(opts.project_root_abs_path)?;

    let target_dir_abs_path = opts.wasm_output_dir_abs_path.join("target");
    let package_dir_abs_path = opts.wasm_output_dir_abs_path.join("package");

    fs::create_dir_all(&target_dir_abs_path).map_err(|e| WasmBuildError::Io {
        path: target_dir_abs_path.clone(),
        source: e,
    })?;
    fs::create_dir_all(&package_dir_abs_path).map_err(|e| WasmBuildError::Io {
        path: package_dir_abs_path.clone(),
        source: e,
    })?;

    builder::build_project(BuildOpts {
        project_root_abs_path: opts.project_root_abs_path,
        target_dir_abs_path: &target_dir_abs_path,
        target_triple: Some(WASM_TARGET_TRIPLE),
        features: &[WASM_CARGO_FEATURE],
        release: opts.release,
    })?;

    let profile_dir = if opts.release { "release" } else { "debug" };
    let wasm_artifact_abs_path = target_dir_abs_path
        .join(WASM_TARGET_TRIPLE)
        .join(profile_dir)
        .join(format!("{lib_name}.wasm"));

    if !wasm_artifact_abs_path.exists() {
        return Err(WasmBuildError::WasmArtifactMissing(wasm_artifact_abs_path));
    }

    let wasm_bindgen_bin = wasm_bindgen_bin_path(opts.tools_dir_abs_path);
    let output = Command::new(&wasm_bindgen_bin)
        .arg(&wasm_artifact_abs_path)
        .arg("--out-dir")
        .arg(&package_dir_abs_path)
        .arg("--target")
        .arg("web")
        .output()
        .map_err(|e| WasmBuildError::Io {
            path: wasm_bindgen_bin,
            source: e,
        })?;

    if !output.status.success() {
        return Err(WasmBuildError::WasmBindgenFailed {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(())
}

fn ensure_wasm_target() -> Result<(), WasmBuildError> {
    let output = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map_err(WasmBuildError::RustupNotAvailable)?;

    if !output.status.success() {
        return Err(WasmBuildError::RustupTargetListFailed {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let installed = String::from_utf8_lossy(&output.stdout);
    if installed
        .lines()
        .any(|line| line.trim() == WASM_TARGET_TRIPLE)
    {
        return Ok(());
    }

    let output = Command::new("rustup")
        .args(["target", "add", WASM_TARGET_TRIPLE])
        .output()
        .map_err(WasmBuildError::RustupNotAvailable)?;

    if !output.status.success() {
        return Err(WasmBuildError::RustupTargetAddFailed {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(())
}

fn ensure_wasm_bindgen_cli(tools_dir_abs_path: &Path) -> Result<(), WasmBuildError> {
    let bin_abs_path = wasm_bindgen_bin_path(tools_dir_abs_path);
    if bin_abs_path.exists() {
        return Ok(());
    }

    let install_root_abs_path = tools_dir_abs_path.join("wasm-bindgen-cli");
    fs::create_dir_all(&install_root_abs_path).map_err(|e| WasmBuildError::Io {
        path: install_root_abs_path.clone(),
        source: e,
    })?;

    let output = Command::new("cargo")
        .args(["install", "wasm-bindgen-cli", "--root"])
        .arg(&install_root_abs_path)
        .output()
        .map_err(WasmBuildError::CargoNotAvailable)?;

    if !output.status.success() {
        return Err(WasmBuildError::WasmBindgenInstallFailed {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(())
}

fn wasm_bindgen_bin_path(tools_dir_abs_path: &Path) -> PathBuf {
    tools_dir_abs_path
        .join("wasm-bindgen-cli")
        .join("bin")
        .join("wasm-bindgen")
}

fn read_lib_name(project_root_abs_path: &Path) -> Result<String, WasmBuildError> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(project_root_abs_path)
        .output()
        .map_err(WasmBuildError::CargoNotAvailable)?;

    if !output.status.success() {
        return Err(WasmBuildError::MetadataFailed {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(WasmBuildError::MetadataParseFailed)?;

    json["packages"]
        .as_array()
        .and_then(|pkgs| pkgs.first())
        .and_then(|pkg| pkg["targets"].as_array())
        .and_then(|targets| {
            targets.iter().find(|t| {
                t["kind"].as_array().is_some_and(|kinds| {
                    kinds
                        .iter()
                        .any(|k| matches!(k.as_str(), Some("lib") | Some("rlib") | Some("cdylib")))
                })
            })
        })
        .and_then(|t| t["name"].as_str())
        .map(|name| name.replace('-', "_"))
        .ok_or(WasmBuildError::NoLibTarget)
}
