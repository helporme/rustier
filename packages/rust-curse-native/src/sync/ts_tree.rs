use std::{
    fs,
    path::{Path, PathBuf},
};

use miette::Diagnostic;

use crate::{compiler::TsTreeCompilationOutput, sync::file::write_if_changed};

pub struct SyncTsTreeOpts<'a> {
    pub out_dir: &'a Path,
    pub package_dir: &'a Path,
    pub tree: TsTreeCompilationOutput<'a>,
}

pub fn sync_ts_tree(opts: SyncTsTreeOpts<'_>) -> Result<(), SyncTsTreeError> {
    ensure_dir(opts.out_dir)?;
    let package_dir = canonicalize(opts.package_dir)?;
    for module in opts.tree.modules {
        let source_path = module.source_path.as_path();
        let Ok(relative) = source_path.strip_prefix(&package_dir) else {
            continue;
        };
        let file_name = relative
            .file_name()
            .ok_or_else(|| SyncTsTreeError::MissingFileName {
                path: relative.to_path_buf(),
            })?;
        let declaration_name = format!("{}.d.ts", file_name.to_string_lossy());
        let sourcemap_name = format!("{declaration_name}.map");
        let output_path = opts
            .out_dir
            .join(relative)
            .with_file_name(&declaration_name);
        let sourcemap_path = output_path.with_file_name(&sourcemap_name);
        if let Some(parent) = output_path.parent() {
            ensure_dir(parent)?;
        }
        let declaration = if module.declaration.is_empty() {
            String::new()
        } else {
            format!(
                "{}\n//# sourceMappingURL={sourcemap_name}\n",
                module.declaration
            )
        };
        write_file(output_path, declaration)?;
        write_file(sourcemap_path, module.sourcemap)?;
    }
    Ok(())
}

fn canonicalize(path: &Path) -> Result<PathBuf, SyncTsTreeError> {
    path.canonicalize()
        .map_err(|source| SyncTsTreeError::Canonicalize {
            path: path.to_path_buf(),
            source,
        })
}

fn ensure_dir(path: &Path) -> Result<(), SyncTsTreeError> {
    fs::create_dir_all(path).map_err(|source| SyncTsTreeError::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })
}

fn write_file(path: PathBuf, contents: impl AsRef<[u8]>) -> Result<(), SyncTsTreeError> {
    write_if_changed(&path, contents)
        .map(|_| ())
        .map_err(|source| SyncTsTreeError::WriteFile { path, source })
}

#[derive(thiserror::Error, Diagnostic, Debug)]
pub enum SyncTsTreeError {
    #[error("failed to canonicalize {}", path.display())]
    Canonicalize {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to create directory {}", path.display())]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write {}", path.display())]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("path has no file name: {}", path.display())]
    MissingFileName { path: PathBuf },
}
