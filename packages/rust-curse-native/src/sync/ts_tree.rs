use std::{
    fs,
    path::{Path, PathBuf},
};

use miette::Diagnostic;

use tracing::{debug, info, trace};

use crate::{codegen, sync::file::write_if_changed};

/// Options for [`sync_ts_tree`].
pub struct SyncTsTreeOpts<'a> {
    /// The directory where mirrored TypeScript declarations and source maps
    /// should be written.
    pub our_dir: &'a Path,
    /// The source `src` directory used to calculate mirrored module paths.
    pub src_dir: &'a Path,
    /// The database for compiling TypeScript declarations.
    pub db: &'a codegen::db::DbImpl,
}

/// Syncs a mirrored `src` tree with TypeScript declarations and source maps
/// for each Rust module.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
///
/// use rust_curse::{
///     codegen::db::DbImpl,
///     sync::ts_tree::{sync_ts_tree, SyncTsTreeOpts},
/// };
///
/// sync_ts_tree(SyncTsTreeOpts {
///     our_dir: Path::new(".rust-curse/ts"),
///     src_dir: Path::new("src"),
///     db: &DbImpl::default(),
/// })?;
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[tracing::instrument(
    level = "debug",
    skip(opts),
    fields(our_dir = %opts.our_dir.display(), src_dir = %opts.src_dir.display())
)]
pub fn sync_ts_tree(opts: SyncTsTreeOpts<'_>) -> Result<(), Error> {
    info!("syncing TypeScript declaration tree");

    ensure_dir(opts.our_dir)?;
    let src_dir = opts
        .src_dir
        .parent()
        .ok_or_else(|| Error::ReadParentDirectory {
            path: opts.src_dir.to_path_buf(),
        })?;

    let src_dir = src_dir
        .canonicalize()
        .map_err(|source| Error::CanonicalizeSourceDirectory {
            path: opts.src_dir.to_path_buf(),
            source,
        })?;

    for mod_id in codegen::rustdoc::all_mod_ids(opts.db) {
        let item = mod_id.item(opts.db);
        let Some(span) = &item.span else {
            trace!("skipping rustdoc module without a span");
            continue;
        };

        let crate_ = mod_id.crate_(opts.db);

        // todo: shared utility for resolving span filename abs path
        let span_path = if span.filename.is_absolute() {
            span.filename.clone()
        } else {
            crate_.dir.join(&span.filename)
        };
        let span_path =
            span_path
                .canonicalize()
                .map_err(|source| Error::CanonicalizeModulePath {
                    path: span_path,
                    source,
                })?;

        let relative_path =
            span_path
                .strip_prefix(&src_dir)
                .map_err(|_| Error::ModuleOutsideSourceDirectory {
                    module_path: span_path.clone(),
                    src_dir: src_dir.clone(),
                })?;

        let Some(file_name) = relative_path.file_name() else {
            return Err(Error::MissingModuleFileName {
                path: relative_path.to_path_buf(),
            });
        };

        let declaration_file_name = format!("{}.d.ts", file_name.to_string_lossy());
        let sourcemap_file_name = format!("{declaration_file_name}.map");
        let output_path = opts
            .our_dir
            .join(relative_path)
            .with_file_name(&declaration_file_name);
        let sourcemap_path = output_path.with_file_name(&sourcemap_file_name);

        if let Some(parent) = output_path.parent() {
            ensure_dir(parent)?;
        }

        let mut declaration = codegen::ts::compile_mod(opts.db, *mod_id).to_string();
        if !declaration.is_empty() {
            declaration.push_str(&format!("\n//# sourceMappingURL={sourcemap_file_name}\n"));
        }
        write_file(output_path, declaration.as_bytes())?;

        let sourcemap = codegen::ts_sourcemap::compile_mod(opts.db, *mod_id).to_string();
        write_file(sourcemap_path, sourcemap.as_bytes())?;
    }

    info!("synced TypeScript declaration tree");

    Ok(())
}

fn ensure_dir(path: &Path) -> Result<(), Error> {
    if path.exists() {
        debug!(path = %path.display(), "directory already exists");
        return Ok(());
    }

    debug!(path = %path.display(), "creating directory");
    fs::create_dir_all(path).map_err(|source| Error::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn write_file(path: PathBuf, content: impl AsRef<[u8]>) -> Result<bool, Error> {
    let changed = write_if_changed(&path, content.as_ref()).map_err(|source| Error::WriteFile {
        path: path.clone(),
        source,
    })?;
    debug!(path = %path.display(), changed, "synced TypeScript tree file");
    Ok(changed)
}

/// Error returned when syncing the TypeScript declaration tree fails.
#[derive(thiserror::Error, Diagnostic, Debug)]
pub enum Error {
    #[error("failed to create directory at {}", path.display())]
    #[diagnostic(code(rust_curse::sync_ts_tree::create_directory))]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to read parent directory at {}", path.display())]
    #[diagnostic(code(rust_curse::sync_ts_tree::canonicalize_source_directory))]
    ReadParentDirectory { path: PathBuf },

    #[error("failed to canonicalize source directory at {}", path.display())]
    #[diagnostic(code(rust_curse::sync_ts_tree::canonicalize_source_directory))]
    CanonicalizeSourceDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to canonicalize module path at {}", path.display())]
    #[diagnostic(code(rust_curse::sync_ts_tree::canonicalize_module_path))]
    CanonicalizeModulePath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "Rust module path {} is outside source directory {}",
        module_path.display(),
        src_dir.display()
    )]
    #[diagnostic(code(rust_curse::sync_ts_tree::module_outside_source_directory))]
    ModuleOutsideSourceDirectory {
        module_path: PathBuf,
        src_dir: PathBuf,
    },

    #[error("Rust module path {} has no file name", path.display())]
    #[diagnostic(code(rust_curse::sync_ts_tree::missing_module_file_name))]
    MissingModuleFileName { path: PathBuf },

    #[error("failed to write TypeScript tree file at {}", path.display())]
    #[diagnostic(code(rust_curse::sync_ts_tree::write_file))]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
