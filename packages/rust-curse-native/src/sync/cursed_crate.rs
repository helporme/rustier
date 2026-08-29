use std::{
    fs,
    path::{Path, PathBuf},
};

use miette::Diagnostic;
use thiserror::Error;

use crate::{
    compiler::Platform,
    sync::manifest,
    tools::{node, workspace},
};

pub struct SyncCrateOpts<'a> {
    pub manifest_path: &'a Path,
    pub out_dir: &'a Path,
    pub platform: Platform,
    pub shim: &'a str,
}

pub fn sync_crate(opts: SyncCrateOpts<'_>) -> Result<bool, SyncCrateError> {
    let package = workspace::get_package(opts.manifest_path)
        .map_err(|source| SyncCrateError::GetPackage { source })?;
    let manifest = manifest::render_manifest(
        opts.platform,
        manifest::ManifestOpts {
            name: &package.name,
            version: &package.version,
            edition: &package.edition,
            source_crate_name: &package.source_crate_name,
            source_package_path: &package.source_package_path,
        },
    );

    let mut changed = ensure_dir(opts.out_dir)?;
    let src_dir = opts.out_dir.join("src");
    changed |= ensure_dir(&src_dir)?;
    changed |= write_file(opts.out_dir.join("Cargo.toml"), manifest)?;
    changed |= write_file(src_dir.join("lib.rs"), opts.shim)?;

    if matches!(opts.platform, Platform::Node) {
        changed |= write_file(
            opts.out_dir.join("build.rs"),
            "fn main() { napi_build::setup(); }\n",
        )?;
        changed |= node::ensure_node_compat(opts.out_dir)
            .map_err(|source| SyncCrateError::EnsureNodeCompat { source })?;
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
    let content = content.as_ref();
    match fs::read(&path) {
        Ok(existing) if existing == content => Ok(false),
        Ok(_) => fs::write(&path, content)
            .map(|()| true)
            .map_err(|source| SyncCrateError::WriteFile { path, source }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::write(&path, content)
            .map(|()| true)
            .map_err(|source| SyncCrateError::WriteFile { path, source }),
        Err(source) => Err(SyncCrateError::WriteFile { path, source }),
    }
}

#[derive(Error, Diagnostic, Debug)]
pub enum SyncCrateError {
    #[error("failed to get source package metadata")]
    GetPackage {
        #[source]
        source: workspace::GetPackageErr,
    },
    #[error("failed to create directory at {}", path.display())]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write generated file at {}", path.display())]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to make generated Node crate compatible with Node tooling")]
    EnsureNodeCompat {
        #[source]
        source: node::EnsureNodeCompatError,
    },
}
