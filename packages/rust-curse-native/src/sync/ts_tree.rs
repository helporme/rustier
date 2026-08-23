use std::{
    fs,
    path::{Path, PathBuf},
};

use miette::Diagnostic;

use crate::{
    compiler::{Compiler, FilePath},
    sync::file::write_if_changed,
};

pub struct SyncTsTreeOpts<'a> {
    pub out_dir: &'a Path,
    pub package_dir: &'a Path,
    pub compiler: &'a Compiler,
}

pub fn sync_ts_tree(opts: SyncTsTreeOpts<'_>) -> Result<(), SyncTsTreeError> {
    ensure_dir(opts.out_dir)?;
    let package_dir = canonicalize(opts.package_dir)?;
    let mut pending = vec![package_dir.clone()];

    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|source| SyncTsTreeError::ReadDirectory {
            path: directory.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| SyncTsTreeError::ReadDirectory {
                path: directory.clone(),
                source,
            })?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|source| SyncTsTreeError::ReadFileType {
                    path: path.clone(),
                    source,
                })?;
            if file_type.is_dir() {
                if !matches!(entry.file_name().to_str(), Some(".git" | "target")) {
                    pending.push(path);
                }
                continue;
            }
            if !file_type.is_file() || path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }

            let source_path = canonicalize(&path)?;
            let compiler_path = FilePath::new_real(path_string(&source_path)?)
                .map_err(SyncTsTreeError::InvalidFilePath)?;
            let Some(mut declaration) = opts.compiler.compile_ts(compiler_path.clone()) else {
                continue;
            };
            let Some(sourcemap) = opts.compiler.compile_ts_sourcemap(compiler_path) else {
                continue;
            };
            let relative = source_path.strip_prefix(&package_dir).map_err(|_| {
                SyncTsTreeError::SourceOutsidePackage {
                    path: source_path.clone(),
                    package_dir: package_dir.clone(),
                }
            })?;
            let file_name =
                relative
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
            if !declaration.is_empty() {
                declaration.push_str(&format!("\n//# sourceMappingURL={sourcemap_name}\n"));
            }
            write_file(output_path, declaration)?;
            write_file(sourcemap_path, sourcemap)?;
        }
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

fn path_string(path: &Path) -> Result<&str, SyncTsTreeError> {
    path.to_str().ok_or_else(|| SyncTsTreeError::NonUtf8Path {
        path: path.to_path_buf(),
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
    #[error("failed to read directory {}", path.display())]
    ReadDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read file type for {}", path.display())]
    ReadFileType {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
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
    #[error("path is not valid UTF-8: {}", path.display())]
    NonUtf8Path { path: PathBuf },
    #[error(
        "source {} is outside package {}",
        path.display(),
        package_dir.display()
    )]
    SourceOutsidePackage { path: PathBuf, package_dir: PathBuf },
    #[error("path has no file name: {}", path.display())]
    MissingFileName { path: PathBuf },
    #[error(transparent)]
    InvalidFilePath(#[from] crate::compiler::FilePathError),
}
