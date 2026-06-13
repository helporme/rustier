//! Locate packages within a Cargo workspace/dependency graph.

use std::path::{Path, PathBuf};

use cargo_metadata::{Metadata, MetadataCommand, Package, Target, TargetKind};
use miette::Diagnostic;
use thiserror::Error;

/// Locate a workspace package or direct dependency by package name.
pub fn locate_dependency(
    manifest_path: &Path,
    package_name: String,
) -> std::result::Result<PathBuf, LocateDependencyErr> {
    let metadata = MetadataCommand::new()
        .manifest_path(manifest_path)
        .exec()
        .map_err(|source| LocateDependencyErr::ReadCargoMetadata {
            manifest_path: manifest_path.to_path_buf(),
            source,
        })?;

    if let Some(package) = metadata
        .workspace_packages()
        .into_iter()
        .find(|package| package_matches_name(package, &package_name))
    {
        return package_manifest_path(package);
    }

    let root = root_package(&metadata, manifest_path)?;
    let package = direct_dependency(&metadata, root, &package_name).ok_or_else(|| {
        LocateDependencyErr::PackageNotFound {
            package_name: package_name.clone(),
            root_package: root.name.to_string(),
        }
    })?;

    package_manifest_path(package)
}

fn root_package<'a>(
    metadata: &'a Metadata,
    manifest_path: &Path,
) -> std::result::Result<&'a Package, LocateDependencyErr> {
    metadata
        .root_package()
        .ok_or_else(|| LocateDependencyErr::MissingRootPackage {
            manifest_path: manifest_path.to_path_buf(),
        })
}

fn direct_dependency<'a>(
    metadata: &'a Metadata,
    root: &Package,
    package_name: &str,
) -> Option<&'a Package> {
    root.dependencies.iter().find_map(|dep| {
        let package = metadata
            .packages
            .iter()
            .find(|package| package.name.as_str() == dep.name)?;
        let renamed = dep.rename.as_deref() == Some(package_name);
        (renamed || package_matches_name(package, package_name)).then_some(package)
    })
}

fn package_matches_name(package: &Package, package_name: &str) -> bool {
    package.name.as_str() == package_name
        || package
            .targets
            .iter()
            .any(|target| is_lib_target(target) && target.name == package_name)
}

fn is_lib_target(target: &Target) -> bool {
    target
        .kind
        .iter()
        .any(|kind| matches!(kind, TargetKind::Lib | TargetKind::ProcMacro))
}

fn package_manifest_path(package: &Package) -> std::result::Result<PathBuf, LocateDependencyErr> {
    package
        .manifest_path
        .as_std_path()
        .canonicalize()
        .map_err(|source| LocateDependencyErr::CanonicalizeManifestPath {
            package: package.name.to_string(),
            source,
        })
}

#[derive(Error, Diagnostic, Debug)]
pub enum LocateDependencyErr {
    #[error("failed to read cargo metadata for {}", manifest_path.display())]
    #[diagnostic(code(rust_curse::workspace::read_cargo_metadata))]
    ReadCargoMetadata {
        manifest_path: PathBuf,
        #[source]
        source: cargo_metadata::Error,
    },

    #[error("package `{package_name}` was not found as a workspace package or a direct dependency of `{root_package}`")]
    #[diagnostic(code(rust_curse::workspace::package_not_found))]
    PackageNotFound {
        package_name: String,
        root_package: String,
    },

    #[error("could not find a root package for manifest {}; virtual workspace manifests can only locate workspace members", manifest_path.display())]
    #[diagnostic(code(rust_curse::workspace::missing_root_package))]
    MissingRootPackage { manifest_path: PathBuf },

    #[error("failed to canonicalize manifest path for {package}")]
    #[diagnostic(code(rust_curse::workspace::canonicalize_manifest_path))]
    CanonicalizeManifestPath {
        package: String,
        #[source]
        source: std::io::Error,
    },
}
