use std::{
    collections::HashSet,
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
    let mut expected_outputs = HashSet::new();
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
        expected_outputs.insert(output_path.clone());
        expected_outputs.insert(sourcemap_path.clone());
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
    remove_stale_outputs(opts.out_dir, &expected_outputs)?;
    Ok(())
}

fn remove_stale_outputs(
    out_dir: &Path,
    expected_outputs: &HashSet<PathBuf>,
) -> Result<(), SyncTsTreeError> {
    let mut pending = vec![out_dir.to_path_buf()];
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
                pending.push(path);
            } else if file_type.is_file()
                && is_generated_output(&path)
                && !expected_outputs.contains(&path)
            {
                fs::remove_file(&path)
                    .map_err(|source| SyncTsTreeError::RemoveFile { path, source })?;
            }
        }
    }
    Ok(())
}

fn is_generated_output(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".rs.d.ts") || name.ends_with(".rs.d.ts.map"))
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
    #[error("failed to remove stale generated file {}", path.display())]
    RemoveFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("path has no file name: {}", path.display())]
    MissingFileName { path: PathBuf },
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::compiler::{FilePath, TsModuleCompilationOutput};

    use super::*;

    #[test]
    fn sync_writes_current_tree_and_removes_stale_generated_files() {
        let temp = tempdir().unwrap();
        let package_dir = temp.path().join("package");
        let source_dir = package_dir.join("src");
        let out_dir = temp.path().join("types");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(out_dir.join("src")).unwrap();

        let source_dir = source_dir.canonicalize().unwrap();
        let source_path = FilePath::new_real(source_dir.join("lib.rs").to_string_lossy()).unwrap();
        fs::write(out_dir.join("src/old.rs.d.ts"), "stale").unwrap();
        fs::write(out_dir.join("src/old.rs.d.ts.map"), "stale").unwrap();
        fs::write(out_dir.join("keep.txt"), "unmanaged").unwrap();

        sync_ts_tree(SyncTsTreeOpts {
            out_dir: &out_dir,
            package_dir: &package_dir,
            tree: TsTreeCompilationOutput {
                modules: vec![TsModuleCompilationOutput {
                    source_path: &source_path,
                    declaration: "export function current(): void;",
                    sourcemap: r#"{"version":3}"#,
                }],
            },
        })
        .unwrap();

        assert!(
            fs::read_to_string(out_dir.join("src/lib.rs.d.ts"))
                .unwrap()
                .contains("export function current")
        );
        assert!(!out_dir.join("src/old.rs.d.ts").exists());
        assert!(!out_dir.join("src/old.rs.d.ts.map").exists());
        assert_eq!(
            fs::read_to_string(out_dir.join("keep.txt")).unwrap(),
            "unmanaged"
        );
    }
}
