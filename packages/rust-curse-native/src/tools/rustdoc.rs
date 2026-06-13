//! Generate and load rustdoc JSON for crates.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use cargo_metadata::{MetadataCommand, Target, TargetKind};
use miette::Diagnostic;
use rustdoc_types::Crate;
use thiserror::Error;

/// Generate rustdoc JSON for the package identified by `manifest_path` and
/// deserialize it.
pub fn run(manifest_path: &Path) -> Result<Crate, RustdocError> {
    let metadata = MetadataCommand::new()
        .manifest_path(manifest_path)
        .exec()
        .map_err(|source| RustdocError::ReadCargoMetadata {
            manifest_path: manifest_path.to_path_buf(),
            source,
        })?;

    let package = metadata
        .root_package()
        .ok_or_else(|| RustdocError::MissingRootPackage {
            manifest_path: manifest_path.to_path_buf(),
        })?;

    let lib_target = package
        .targets
        .iter()
        .find(|target| is_lib_target(target))
        .ok_or_else(|| RustdocError::MissingLibraryTarget {
            package: package.name.to_string(),
        })?;

    let target_dir = metadata
        .target_directory
        .join("rustdoc-json")
        .into_std_path_buf();

    let output = Command::new("cargo")
        .args(["+nightly", "rustdoc", "--manifest-path"])
        .arg(package.manifest_path.as_std_path())
        .arg("--target-dir")
        .arg(&target_dir)
        .args([
            "--lib",
            "--",
            "-Z",
            "unstable-options",
            "--output-format",
            "json",
        ])
        .output()
        .map_err(|source| RustdocError::RunCargoRustdoc { source })?;

    ensure_success(
        &output,
        &format!("cargo +nightly rustdoc failed for {}", package.name),
    )?;

    let rustdoc_json_path = target_dir
        .join("doc")
        .join(format!("{}.json", &lib_target.name.replace('-', "_")));

    let json =
        fs::read_to_string(&rustdoc_json_path).map_err(|source| RustdocError::ReadRustdocJson {
            path: rustdoc_json_path.clone(),
            source,
        })?;

    serde_json::from_str(&json).map_err(|source| RustdocError::DeserializeRustdocJson {
        path: rustdoc_json_path.clone(),
        source,
    })
}

/// Ensure the default nightly toolchain required by rustdoc JSON generation is
/// installed.
pub fn ensure() -> Result<(), RustdocError> {
    let check = Command::new("rustup")
        .args(["run", "nightly", "rustdoc", "--version"])
        .output()
        .map_err(|source| RustdocError::CheckNightlyRustdoc { source })?;

    if check.status.success() {
        return Ok(());
    }

    let output = Command::new("rustup")
        .args(["toolchain", "install", "nightly"])
        .output()
        .map_err(|source| RustdocError::InstallNightlyToolchain { source })?;

    ensure_success(&output, "`rustup toolchain install nightly` failed")
}

fn is_lib_target(target: &Target) -> bool {
    target
        .kind
        .iter()
        .any(|kind| matches!(kind, TargetKind::Lib | TargetKind::ProcMacro))
}

fn ensure_success(output: &Output, context: &str) -> Result<(), RustdocError> {
    if output.status.success() {
        return Ok(());
    }

    Err(RustdocError::CommandFailed {
        context: context.to_string(),
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string(),
        stderr: String::from_utf8_lossy(&output.stderr)
            .trim_end()
            .to_string(),
    })
}

#[derive(Error, Diagnostic, Debug)]
pub enum RustdocError {
    #[error("failed to read cargo metadata for {}", manifest_path.display())]
    #[diagnostic(code(rust_curse::rustdoc::read_cargo_metadata))]
    ReadCargoMetadata {
        manifest_path: PathBuf,
        #[source]
        source: cargo_metadata::Error,
    },

    #[error("package `{package}` does not have a library target")]
    #[diagnostic(code(rust_curse::rustdoc::missing_library_target))]
    MissingLibraryTarget { package: String },

    #[error("failed to run cargo +nightly rustdoc")]
    #[diagnostic(code(rust_curse::rustdoc::run_cargo_rustdoc))]
    RunCargoRustdoc {
        #[source]
        source: std::io::Error,
    },

    #[error("failed to read generated rustdoc JSON at {}", path.display())]
    #[diagnostic(code(rust_curse::rustdoc::read_rustdoc_json))]
    ReadRustdocJson {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to deserialize rustdoc JSON at {}", path.display())]
    #[diagnostic(code(rust_curse::rustdoc::deserialize_rustdoc_json))]
    DeserializeRustdocJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to run `rustup run nightly rustdoc --version`; is rustup installed?")]
    #[diagnostic(code(rust_curse::rustdoc::check_nightly_rustdoc))]
    CheckNightlyRustdoc {
        #[source]
        source: std::io::Error,
    },

    #[error("failed to run `rustup toolchain install nightly`")]
    #[diagnostic(code(rust_curse::rustdoc::install_nightly_toolchain))]
    InstallNightlyToolchain {
        #[source]
        source: std::io::Error,
    },

    #[error("could not find a root package for manifest {}; virtual workspace manifests cannot be documented directly", manifest_path.display())]
    #[diagnostic(code(rust_curse::rustdoc::missing_root_package))]
    MissingRootPackage { manifest_path: PathBuf },

    #[error("{context} (exit code {exit_code:?})\nstdout:\n{stdout}\nstderr:\n{stderr}")]
    #[diagnostic(code(rust_curse::rustdoc::command_failed))]
    CommandFailed {
        context: String,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },
}
