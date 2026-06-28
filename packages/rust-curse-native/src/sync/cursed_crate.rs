use std::{
    fs,
    path::{Path, PathBuf},
};

use miette::Diagnostic;
use thiserror::Error;

use crate::{
    codegen,
    sync::{file::write_if_changed, manifest},
    tools::workspace,
};

/// Options for [`sync_crate`].
pub struct SyncCrateOps<'a> {
    /// The path to the Cargo.toml manifest file of a crate which should be
    /// synced.
    pub manifest_path: &'a Path,
    /// The path to the directory where the cursed crate will be crated.
    /// e.g. ".rust-curse/crates/"
    pub out_dir: &'a Path,
    /// The API to use for compiling bindings.
    pub api: codegen::bindings::Api,
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
///     codegen::{bindings::Api, db::DbImpl},
///     sync::cursed_crate::{sync_crate, SyncCrateOps},
/// };
///
/// let changed = sync_crate(SyncCrateOps {
///     manifest_path: Path::new("Cargo.toml"),
///     out_dir: Path::new(".rust-curse/crates"),
///     api: Api::Wasm,
///     db: DbImpl::default(),
/// })?;
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn sync_crate(opts: SyncCrateOps) -> Result<bool, SyncCrateError> {
    // todo: remove, use package passed in opts
    let package = workspace::get_package(opts.manifest_path)
        .map_err(|source| SyncCrateError::GetPackage { source })?;

    let manifest = manifest::render_manifest(manifest::ManifestOpts {
        api: opts.api,
        name: &package.name,
        version: &package.version,
        edition: &package.edition,
        source_crate_name: &package.source_crate_name,
        source_package_path: &package.source_package_path,
    });

    let bindings = codegen::bindings::compile(opts.db, opts.api);

    let mut changed = ensure_dir(opts.out_dir)?;
    let src_dir = opts.out_dir.join("src");
    changed |= ensure_dir(&src_dir)?;
    changed |= write_file(opts.out_dir.join("Cargo.toml"), manifest)?;
    changed |= write_file(src_dir.join("lib.rs"), bindings)?;

    if let Some(build_script_body) = opts.api.build_script_body() {
        let build_script = format!("fn main() {{\n\t{build_script_body}\n}}\n");
        changed |= write_file(opts.out_dir.join("build.rs"), build_script)?;
    }

    Ok(changed)
}

fn ensure_dir(path: &Path) -> Result<bool, SyncCrateError> {
    if path.exists() {
        return Ok(false);
    }

    fs::create_dir_all(path).map_err(|source| SyncCrateError::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(true)
}

fn write_file(path: PathBuf, content: impl AsRef<[u8]>) -> Result<bool, SyncCrateError> {
    write_if_changed(&path, content).map_err(|source| SyncCrateError::WriteFile { path, source })
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
}
