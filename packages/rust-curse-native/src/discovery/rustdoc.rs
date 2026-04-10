use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RustdocError {
    #[error("cargo is not available")]
    CargoNotAvailable(#[source] io::Error),

    #[error("cargo metadata failed (exit code {exit_code:?}):\n{stderr}")]
    MetadataFailed {
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("failed to parse cargo metadata output")]
    MetadataParseFailed(#[source] serde_json::Error),

    #[error("no library target found in cargo metadata")]
    NoLibTarget,

    #[error("cargo rustdoc failed (exit code {exit_code:?}):\n{stderr}")]
    RustdocFailed {
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("rustdoc JSON not found at '{}'", .0.display())]
    JsonNotFound(PathBuf),

    #[error("failed to read rustdoc JSON from '{}'", .0.display())]
    JsonReadFailed(PathBuf, #[source] io::Error),

    #[error("failed to parse rustdoc JSON")]
    JsonParseFailed(#[source] serde_json::Error),
}

/// Generates rustdoc in json format.
///
/// Requires nightly version of rust installed.
pub fn generate_rustdoc(
    project_root_abs_path: &Path,
) -> Result<rustdoc_types::Crate, RustdocError> {
    let meta = cargo_metadata(project_root_abs_path)?;

    let output = Command::new("cargo")
        .args([
            "+nightly",
            "rustdoc",
            "--lib",
            "--",
            "-Z",
            "unstable-options",
            "--output-format",
            "json",
        ])
        .current_dir(project_root_abs_path)
        .output()
        .map_err(RustdocError::CargoNotAvailable)?;

    if !output.status.success() {
        return Err(RustdocError::RustdocFailed {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let json_abs_path = Path::new(&meta.target_dir)
        .join("doc")
        .join(format!("{}.json", meta.lib_name));

    if !json_abs_path.exists() {
        return Err(RustdocError::JsonNotFound(json_abs_path));
    }

    let json = fs::read_to_string(&json_abs_path)
        .map_err(|e| RustdocError::JsonReadFailed(json_abs_path, e))?;

    serde_json::from_str(&json).map_err(RustdocError::JsonParseFailed)
}

struct CrateMetadata {
    /// Absolute path to the target directory (shared in workspaces).
    target_dir: String,
    /// Library target name (hyphens already replaced with underscores).
    lib_name: String,
}

fn cargo_metadata(abs_cargo_dir: &Path) -> Result<CrateMetadata, RustdocError> {
    let output = Command::new("cargo")
        .args(["metadata", "--frozen", "--format-version", "1", "--no-deps"])
        .current_dir(abs_cargo_dir)
        .output()
        .map_err(RustdocError::CargoNotAvailable)?;

    if !output.status.success() {
        return Err(RustdocError::MetadataFailed {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(RustdocError::MetadataParseFailed)?;

    let target_dir = json["target_directory"]
        .as_str()
        .ok_or(RustdocError::NoLibTarget)?
        .to_owned();

    // Find the library target name from the first package's targets.
    let lib_name = json["packages"]
        .as_array()
        .and_then(|pkgs| pkgs.first())
        .and_then(|pkg| pkg["targets"].as_array())
        .and_then(|targets| {
            targets.iter().find(|t| {
                t["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|k| k.as_str() == Some("lib")))
            })
        })
        .and_then(|t| t["name"].as_str())
        .ok_or(RustdocError::NoLibTarget)?
        .to_owned();

    Ok(CrateMetadata {
        target_dir,
        lib_name,
    })
}
