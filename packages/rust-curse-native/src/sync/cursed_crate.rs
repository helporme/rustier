use std::{
    fs,
    path::{Path, PathBuf},
};

use miette::Diagnostic;
use thiserror::Error;
use tracing::{debug, info};

use crate::{
    codegen::{self, bindings::ApiKind},
    sync::{file::write_if_changed, manifest},
    tools::{node, workspace},
};

/// Options for [`sync_crate`].
pub struct SyncCrateOps<'a> {
    /// The path to the Cargo.toml manifest file of a crate which should be
    /// synced.
    pub manifest_path: &'a Path,
    /// The path to the directory where the cursed crate will be crated.
    /// e.g. ".rust-curse/crates/"
    pub out_dir: &'a Path,
    /// The database for compiling bindings.
    pub db: &'a codegen::db::DbImpl,
}

/// Syncs a generated cursed crate for a source crate.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
///
/// use rust_curse::{
///     codegen::{
///         bindings::{Api, ApiKind, WasmTarget},
///         db::DbImpl,
///     },
///     sync::cursed_crate::{sync_crate, SyncCrateOps},
/// };
///
/// let mut db = DbImpl::default();
/// Api::set(
///     &mut db,
///     ApiKind::Wasm {
///         target: WasmTarget::Web,
///     },
/// );
///
/// let changed = sync_crate(SyncCrateOps {
///     manifest_path: Path::new("Cargo.toml"),
///     out_dir: Path::new(".rust-curse/crates"),
///     db: &db,
/// })?;
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[tracing::instrument(
    level = "debug",
    skip(opts),
    fields(
        manifest_path = %opts.manifest_path.display(),
        out_dir = %opts.out_dir.display(),
    )
)]
pub fn sync_crate(opts: SyncCrateOps) -> Result<bool, SyncCrateError> {
    info!("syncing generated cursed crate");

    // todo: remove, use package passed in opts
    let package = workspace::get_package(opts.manifest_path)
        .map_err(|source| SyncCrateError::GetPackage { source })?;

    debug!(
        package = %package.name,
        version = %package.version,
        edition = %package.edition,
        source_crate_name = %package.source_crate_name,
        source_package_path = %package.source_package_path.display(),
        "resolved source package metadata"
    );

    let api = codegen::bindings::Api::get(opts.db);
    let manifest = manifest::render_manifest(
        opts.db,
        manifest::ManifestOpts {
            name: &package.name,
            version: &package.version,
            edition: &package.edition,
            source_crate_name: &package.source_crate_name,
            source_package_path: &package.source_package_path,
        },
    );

    debug!("compiled generated crate manifest");

    let bindings = codegen::bindings::compile(opts.db);
    debug!(
        bindings_bytes = bindings.len(),
        "compiled generated bindings"
    );

    let mut changed = ensure_dir(opts.out_dir)?;
    let src_dir = opts.out_dir.join("src");
    changed |= ensure_dir(&src_dir)?;
    changed |= write_file(opts.out_dir.join("Cargo.toml"), manifest)?;
    changed |= write_file(src_dir.join("lib.rs"), bindings)?;

    if let Some(build_script_body) = api.build_script_body(opts.db) {
        debug!("writing generated build script");
        let build_script = format!("fn main() {{\n\t{build_script_body}\n}}\n");
        changed |= write_file(opts.out_dir.join("build.rs"), build_script)?;
    }

    if api.kind(opts.db) == ApiKind::Node {
        debug!("ensuring generated Node crate compatibility");
        changed |= node::ensure_node_compat(opts.out_dir)
            .map_err(|source| SyncCrateError::EnsureNodeCompat { source })?;
    }

    info!(changed, "synced generated cursed crate");

    Ok(changed)
}

fn ensure_dir(path: &Path) -> Result<bool, SyncCrateError> {
    if path.exists() {
        debug!(path = %path.display(), "directory already exists");
        return Ok(false);
    }

    debug!(path = %path.display(), "creating directory");
    fs::create_dir_all(path).map_err(|source| SyncCrateError::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(true)
}

fn write_file(path: PathBuf, content: impl AsRef<[u8]>) -> Result<bool, SyncCrateError> {
    let changed =
        write_if_changed(&path, content.as_ref()).map_err(|source| SyncCrateError::WriteFile {
            path: path.clone(),
            source,
        })?;
    debug!(path = %path.display(), changed, "synced generated file");
    Ok(changed)
}

/// Error returned when syncing a generated cursed crate fails.
#[derive(Error, Diagnostic, Debug)]
pub enum SyncCrateError {
    #[error("failed to get source package metadata")]
    #[diagnostic(code(rust_curse::sync_crate::get_package))]
    GetPackage {
        #[source]
        source: workspace::GetPackageErr,
    },

    #[error("failed to create directory at {}", path.display())]
    #[diagnostic(code(rust_curse::sync_crate::create_directory))]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write generated file at {}", path.display())]
    #[diagnostic(code(rust_curse::sync_crate::write_file))]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to make generated Node crate compatible with Node tooling")]
    #[diagnostic(code(rust_curse::sync_crate::ensure_node_compat))]
    EnsureNodeCompat {
        #[source]
        source: node::EnsureNodeCompatError,
    },
}
