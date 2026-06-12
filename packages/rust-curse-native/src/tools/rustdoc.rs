//! Generate and load rustdoc JSON for crates.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use anyhow::{anyhow, bail, Context, Result};
use cargo_metadata::{Metadata, MetadataCommand, Package, Target, TargetKind};
use rustdoc_types::Crate as RustdocCrate;

/// Options for [`run`].
#[derive(Debug, Clone)]
pub struct RunOpts {
    /// Path to the `Cargo.toml`.
    pub manifest_path: PathBuf,
    /// Package/dependency/crate name to document.
    ///
    /// If omitted, the package at [`Self::manifest_path`] is documented.
    pub crate_name: Option<String>,
}

/// Generate rustdoc JSON for the resolved crate and deserialize it.
///
/// Resolution rules:
///
/// 1. If `crate_name` is omitted, document the package identified by
///    `manifest_path`.
/// 2. If `crate_name` is present, first look for a matching workspace package.
/// 3. Otherwise, look for a matching direct dependency of the package
///    identified by `manifest_path`.
///
/// Matching accepts both Cargo package names and the crate name of the library
/// target, so `foo-bar` and `foo_bar` can both resolve to the same library.
pub fn run(opts: RunOpts) -> Result<RustdocCrate> {
    let metadata = MetadataCommand::new()
        .manifest_path(&opts.manifest_path)
        .exec()
        .with_context(|| {
            format!(
                "failed to read cargo metadata for {}",
                opts.manifest_path.display()
            )
        })?;

    let package = resolve_package(&metadata, &opts.manifest_path, opts.crate_name.as_deref())?;

    let lib_target = package
        .targets
        .iter()
        .find(|target| is_lib_target(target))
        .ok_or_else(|| anyhow!("package `{}` does not have a library target", package.name))?;

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
        .context("failed to run cargo +nightly rustdoc")?;

    ensure_success(
        &output,
        &format!("cargo +nightly rustdoc failed for {}", package.name),
    )?;

    let rustdoc_json_path = target_dir
        .join("doc")
        .join(format!("{}.json", &lib_target.name.replace('-', "_")));
    let json = fs::read_to_string(&rustdoc_json_path).with_context(|| {
        format!(
            "failed to read generated rustdoc JSON at {}",
            rustdoc_json_path.display()
        )
    })?;

    serde_json::from_str(&json).with_context(|| {
        format!(
            "failed to deserialize rustdoc JSON at {}",
            rustdoc_json_path.display()
        )
    })
}

/// Ensure the default nightly toolchain required by rustdoc JSON generation is
/// installed.
pub fn ensure() -> Result<()> {
    let check = Command::new("rustup")
        .args(["run", "nightly", "rustdoc", "--version"])
        .output()
        .context("failed to run `rustup run nightly rustdoc --version`; is rustup installed?")?;

    if check.status.success() {
        return Ok(());
    }

    let output = Command::new("rustup")
        .args(["toolchain", "install", "nightly"])
        .output()
        .context("failed to run `rustup toolchain install nightly`")?;

    ensure_success(&output, "`rustup toolchain install nightly` failed")
}

fn resolve_package<'a>(
    metadata: &'a Metadata,
    manifest_path: &Path,
    crate_name: Option<&str>,
) -> Result<&'a Package> {
    let Some(crate_name) = crate_name else {
        return root_package(metadata, manifest_path);
    };

    if let Some(package) = metadata
        .workspace_packages()
        .into_iter()
        .find(|package| package_matches_name(package, crate_name))
    {
        return Ok(package);
    }

    let root = root_package(metadata, manifest_path)?;
    direct_dependency(metadata, root, crate_name).ok_or_else(|| {
        anyhow!(
            "crate `{crate_name}` was not found as a workspace package or a direct dependency of `{}`",
            root.name
        )
    })
}

fn root_package<'a>(metadata: &'a Metadata, manifest_path: &Path) -> Result<&'a Package> {
    metadata.root_package().ok_or_else(|| {
        anyhow!(
            "could not find a package for manifest {}; virtual workspaces require `crate_name`",
            manifest_path.display()
        )
    })
}

fn direct_dependency<'a>(
    metadata: &'a Metadata,
    root: &Package,
    crate_name: &str,
) -> Option<&'a Package> {
    root.dependencies.iter().find_map(|dep| {
        let package = metadata
            .packages
            .iter()
            .find(|package| package.name.as_str() == dep.name)?;
        let renamed = dep.rename.as_deref() == Some(crate_name);
        (renamed || package_matches_name(package, crate_name)).then_some(package)
    })
}

fn package_matches_name(package: &Package, crate_name: &str) -> bool {
    package.name == crate_name
        || package
            .targets
            .iter()
            .any(|target| is_lib_target(target) && target.name == crate_name)
}

fn is_lib_target(target: &Target) -> bool {
    target
        .kind
        .iter()
        .any(|kind| matches!(kind, TargetKind::Lib | TargetKind::ProcMacro))
}

fn ensure_success(output: &Output, context: &str) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!(
        "{context} (exit code {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        stdout.trim_end(),
        stderr.trim_end()
    )
}
