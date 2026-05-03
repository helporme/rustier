//! Utilities to build the project.

pub mod wasm;

use std::io;
use std::path::Path;
use std::process::Command;

use thiserror::Error;

#[derive(Debug)]
pub struct BuildOpts<'a> {
    /// Absolute path to the project root containing the Cargo.toml to build.
    pub project_root_abs_path: &'a Path,
    /// Absolute path passed via `CARGO_TARGET_DIR`.
    pub target_dir_abs_path: &'a Path,
    /// Optional `--target <triple>`, e.g. `wasm32-unknown-unknown`.
    pub target_triple: Option<&'a str>,
    /// Cargo features to enable, joined with `,` and passed as `--features`.
    pub features: &'a [&'a str],
    /// Build with `--release` when true.
    pub release: bool,
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("cargo is not available")]
    CargoNotAvailable(#[source] io::Error),

    #[error("cargo build failed (exit code {exit_code:?}):\n{stderr}")]
    BuildFailed {
        exit_code: Option<i32>,
        stderr: String,
    },
}

pub fn build_project(opts: BuildOpts<'_>) -> Result<(), BuildError> {
    let mut args: Vec<String> = vec!["build".into()];
    if opts.release {
        args.push("--release".into());
    }
    if let Some(triple) = opts.target_triple {
        args.push("--target".into());
        args.push(triple.into());
    }
    if !opts.features.is_empty() {
        args.push("--features".into());
        args.push(opts.features.join(","));
    }

    let output = Command::new("cargo")
        .args(&args)
        .env("CARGO_TARGET_DIR", opts.target_dir_abs_path)
        .current_dir(opts.project_root_abs_path)
        .output()
        .map_err(BuildError::CargoNotAvailable)?;

    if !output.status.success() {
        return Err(BuildError::BuildFailed {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(())
}
