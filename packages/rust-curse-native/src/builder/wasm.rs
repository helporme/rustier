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
const WASM_BINDGEN_OUT_NAME: &str = "package";

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

/// Ensure `wasm-bindgen-cli` is installed under the given installation
/// directory.
///
/// Returns the absolute path to the `wasm-bindgen` binary.
pub fn ensure_bindgen_tool_installed(
    installation_abs_path: &Path,
) -> Result<PathBuf, WasmBuildError> {
    let bin_abs_path = installation_abs_path.join("bin").join("wasm-bindgen");

    if bin_abs_path.exists() {
        return Ok(bin_abs_path);
    }

    fs::create_dir_all(installation_abs_path).map_err(|e| WasmBuildError::Io {
        path: installation_abs_path.into(),
        source: e,
    })?;

    let output = Command::new("cargo")
        .args(["install", "wasm-bindgen-cli", "--root"])
        .arg(installation_abs_path)
        .output()
        .map_err(WasmBuildError::CargoNotAvailable)?;

    if !output.status.success() {
        return Err(WasmBuildError::WasmBindgenInstallFailed {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(bin_abs_path)
}

/// Ensures the WASM target is installed via `rustup`.
pub fn ensure_wasm_target_installed() -> Result<(), WasmBuildError> {
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

pub struct WasmBuildOpts<'a> {
    /// Project containing the wasm-feature-enabled Cargo.toml.
    pub project_root_abs_path: &'a Path,
    /// Build output directory absolute path
    pub out_dir_abs_path: &'a Path,
    /// Absolute path to the `wasm-bindgen` binary.
    pub wasm_bindgen_tool_bin_abs_path: &'a Path,
    /// Profile name used as the output subdirectory and as a cargo feature.
    pub profile_name: &'a str,
    /// Do we release?
    pub release: bool,
}

/// Paths to generated wasm output files.
#[derive(Debug, Clone)]
pub struct WasmBuildOutput {
    pub js_output_abs_path: PathBuf,
    pub ts_output_abs_path: PathBuf,
    pub wasm_output_abs_path: PathBuf,
    pub wasm_ts_output_abs_path: PathBuf,
}

/// Builds the wasm binary for `profile_name` using `wasm-bindgen`.
pub fn build(opts: WasmBuildOpts<'_>) -> Result<WasmBuildOutput, WasmBuildError> {
    let profile_dir_abs_path = opts.out_dir_abs_path.join(opts.profile_name);
    // cargo target directory
    let target_dir_abs_path = profile_dir_abs_path.join("target");
    // output directory for wasm-bindgen artifacts
    let package_dir_abs_path = profile_dir_abs_path.join("package");

    fs::create_dir_all(&target_dir_abs_path).map_err(|e| WasmBuildError::Io {
        path: target_dir_abs_path.clone(),
        source: e,
    })?;
    fs::create_dir_all(&package_dir_abs_path).map_err(|e| WasmBuildError::Io {
        path: package_dir_abs_path.clone(),
        source: e,
    })?;

    let features: [&str; 2] = [WASM_CARGO_FEATURE, opts.profile_name];
    builder::build_project(BuildOpts {
        project_root_abs_path: opts.project_root_abs_path,
        target_dir_abs_path: &target_dir_abs_path,
        target_triple: Some(WASM_TARGET_TRIPLE),
        features: &features,
        release: opts.release,
    })?;

    let lib_name = read_lib_name(opts.project_root_abs_path)?;
    let profile_dir = if opts.release { "release" } else { "debug" };
    let wasm_artifact_abs_path = target_dir_abs_path
        .join(WASM_TARGET_TRIPLE)
        .join(profile_dir)
        .join(format!("{lib_name}.wasm"));

    if !wasm_artifact_abs_path.exists() {
        return Err(WasmBuildError::WasmArtifactMissing(wasm_artifact_abs_path));
    }

    let output = Command::new(opts.wasm_bindgen_tool_bin_abs_path)
        .arg(&wasm_artifact_abs_path)
        .arg("--out-dir")
        .arg(&package_dir_abs_path)
        .arg("--out-name")
        .arg(WASM_BINDGEN_OUT_NAME)
        .arg("--target")
        .arg("web")
        .output()
        .map_err(|e| WasmBuildError::Io {
            path: opts.wasm_bindgen_tool_bin_abs_path.to_path_buf(),
            source: e,
        })?;

    if !output.status.success() {
        return Err(WasmBuildError::WasmBindgenFailed {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(WasmBuildOutput {
        js_output_abs_path: package_dir_abs_path.join("package.js"),
        ts_output_abs_path: package_dir_abs_path.join("package.d.ts"),
        wasm_output_abs_path: package_dir_abs_path.join("package_bg.wasm"),
        wasm_ts_output_abs_path: package_dir_abs_path.join("package_bg.wasm.d.ts"),
    })
}

/// Reads the lib name from the cargo metadata.
// todo: use cargo_metadata lib
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
