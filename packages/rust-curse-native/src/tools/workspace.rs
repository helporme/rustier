//! Locate packages within a Cargo workspace/dependency graph.

use std::path::{Path, PathBuf};

use cargo_metadata::{Metadata, MetadataCommand, Package as CargoPackage, Target, TargetKind};
use miette::Diagnostic;
use thiserror::Error;
use tracing::debug;

/// Cargo package metadata needed by generated cursed crates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
    /// The Cargo package name.
    pub name: String,
    /// The Cargo package version.
    pub version: String,
    /// The Rust edition used by the package.
    pub edition: String,
    /// The source crate name to use in generated Rust dependencies.
    pub source_crate_name: String,
    /// The canonical source package directory.
    pub source_package_path: PathBuf,
}

/// Reads metadata for the root package identified by `manifest_path`.
#[tracing::instrument(level = "debug", skip_all, fields(manifest_path = %manifest_path.display()))]
pub fn get_package(manifest_path: &Path) -> std::result::Result<Package, GetPackageErr> {
    debug!("reading root package metadata");

    let metadata = MetadataCommand::new()
        .manifest_path(manifest_path)
        .exec()
        .map_err(|source| GetPackageErr::ReadCargoMetadata {
            manifest_path: manifest_path.to_path_buf(),
            source,
        })?;

    let package = metadata
        .root_package()
        .ok_or_else(|| GetPackageErr::MissingRootPackage {
            manifest_path: manifest_path.to_path_buf(),
        })?;
    let lib_target = package
        .targets
        .iter()
        .find(|target| is_lib_target(target))
        .ok_or_else(|| GetPackageErr::MissingLibraryTarget {
            package: package.name.to_string(),
        })?;
    debug!(
        package = %package.name,
        lib_target = %lib_target.name,
        "resolved package library target"
    );

    let source_package_path = package
        .manifest_path
        .as_std_path()
        .parent()
        .ok_or_else(|| GetPackageErr::ManifestHasNoParent {
            manifest_path: package.manifest_path.as_std_path().to_path_buf(),
        })?
        .canonicalize()
        .map_err(|source| GetPackageErr::CanonicalizePackageDir {
            package: package.name.to_string(),
            source,
        })?;

    debug!(
        package = %package.name,
        version = %package.version,
        edition = %package.edition,
        source_crate_name = %lib_target.name,
        source_package_path = %source_package_path.display(),
        "resolved package metadata"
    );

    Ok(Package {
        name: package.name.to_string(),
        version: package.version.to_string(),
        edition: package.edition.to_string(),
        source_crate_name: lib_target.name.clone(),
        source_package_path,
    })
}

/// Locate a workspace package or direct dependency by package name.
#[tracing::instrument(
    level = "debug",
    skip_all,
    fields(manifest_path = %manifest_path.display(), package_name = %package_name)
)]
pub fn locate_dependency(
    manifest_path: &Path,
    package_name: String,
) -> std::result::Result<PathBuf, LocateDependencyErr> {
    debug!("locating Cargo dependency");

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
        debug!(package = %package.name, "found dependency as workspace package");
        return package_manifest_path(package);
    }

    let root = root_package(&metadata, manifest_path)?;
    let package = direct_dependency(&metadata, root, &package_name).ok_or_else(|| {
        LocateDependencyErr::PackageNotFound {
            package_name: package_name.clone(),
            root_package: root.name.to_string(),
        }
    })?;

    debug!(
        package = %package.name,
        root_package = %root.name,
        "found dependency as direct dependency"
    );

    package_manifest_path(package)
}

fn root_package<'a>(
    metadata: &'a Metadata,
    manifest_path: &Path,
) -> std::result::Result<&'a CargoPackage, LocateDependencyErr> {
    metadata
        .root_package()
        .ok_or_else(|| LocateDependencyErr::MissingRootPackage {
            manifest_path: manifest_path.to_path_buf(),
        })
}

fn direct_dependency<'a>(
    metadata: &'a Metadata,
    root: &CargoPackage,
    package_name: &str,
) -> Option<&'a CargoPackage> {
    root.dependencies.iter().find_map(|dep| {
        let package = metadata
            .packages
            .iter()
            .find(|package| package.name.as_str() == dep.name)?;
        let renamed = dep.rename.as_deref() == Some(package_name);
        (renamed || package_matches_name(package, package_name)).then_some(package)
    })
}

fn package_matches_name(package: &CargoPackage, package_name: &str) -> bool {
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

fn package_manifest_path(
    package: &CargoPackage,
) -> std::result::Result<PathBuf, LocateDependencyErr> {
    let manifest_path = package
        .manifest_path
        .as_std_path()
        .canonicalize()
        .map_err(|source| LocateDependencyErr::CanonicalizeManifestPath {
            package: package.name.to_string(),
            source,
        })?;

    debug!(
        package = %package.name,
        manifest_path = %manifest_path.display(),
        "resolved dependency manifest path"
    );

    Ok(manifest_path)
}

#[derive(Error, Diagnostic, Debug)]
pub enum GetPackageErr {
    #[error("failed to read cargo metadata for {}", manifest_path.display())]
    #[diagnostic(code(rust_curse::workspace::read_cargo_metadata))]
    ReadCargoMetadata {
        manifest_path: PathBuf,
        #[source]
        source: cargo_metadata::Error,
    },

    #[error("could not find a root package for manifest {}; virtual workspace manifests cannot be inspected directly", manifest_path.display())]
    #[diagnostic(code(rust_curse::workspace::missing_root_package))]
    MissingRootPackage { manifest_path: PathBuf },

    #[error("package `{package}` does not have a library target")]
    #[diagnostic(code(rust_curse::workspace::missing_library_target))]
    MissingLibraryTarget { package: String },

    #[error("manifest path {} has no parent directory", manifest_path.display())]
    #[diagnostic(code(rust_curse::workspace::manifest_has_no_parent))]
    ManifestHasNoParent { manifest_path: PathBuf },

    #[error("failed to canonicalize package directory for {package}")]
    #[diagnostic(code(rust_curse::workspace::canonicalize_package_dir))]
    CanonicalizePackageDir {
        package: String,
        #[source]
        source: std::io::Error,
    },
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
