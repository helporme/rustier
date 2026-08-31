use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use miette::Diagnostic;

use crate::compiler::Compiler;

use super::write_file::write_file;

#[derive(Debug, Clone)]
pub struct ServerTsTreeOpts {
    /// Folder where the crate .d.ts file tree is generated.
    pub types_out_dir: PathBuf,
    /// Folder where the manifest is stored.
    pub package_dir: PathBuf,
}

pub struct ServerTsTree {
    opts: ServerTsTreeOpts,
}

impl ServerTsTree {
    pub fn new(opts: ServerTsTreeOpts) -> Self {
        Self { opts }
    }

    /// Synchronizes the compiler's TypeScript declaration tree to disk.
    pub fn sync(&self, compiler: &mut Compiler) -> Result<(), ServerTsTreeError> {
        let package_dir = self.opts.package_dir.canonicalize().map_err(|source| {
            ServerTsTreeError::Canonicalize {
                path: self.opts.package_dir.clone(),
                source,
            }
        })?;
        let mut expected_outputs = HashSet::new();

        for module in compiler.compile_ts_tree().modules {
            let Ok(relative) = module.source_path.as_path().strip_prefix(&package_dir) else {
                continue;
            };
            let file_name =
                relative
                    .file_name()
                    .ok_or_else(|| ServerTsTreeError::MissingFileName {
                        path: relative.to_path_buf(),
                    })?;
            let declaration_name = format!("{}.d.ts", file_name.to_string_lossy());
            let sourcemap_name = format!("{declaration_name}.map");
            let output_path = self
                .opts
                .types_out_dir
                .join(relative)
                .with_file_name(&declaration_name);
            let sourcemap_path = output_path.with_file_name(&sourcemap_name);
            expected_outputs.insert(output_path.clone());
            expected_outputs.insert(sourcemap_path.clone());
            let declaration = if module.declaration.is_empty() {
                String::new()
            } else {
                format!(
                    "{}\n//# sourceMappingURL={sourcemap_name}\n",
                    module.declaration
                )
            };
            write_generated_file(output_path, declaration)?;
            write_generated_file(sourcemap_path, module.sourcemap)?;
        }

        remove_stale_outputs(&self.opts.types_out_dir, &expected_outputs)
    }
}

fn remove_stale_outputs(
    out_dir: &Path,
    expected_outputs: &HashSet<PathBuf>,
) -> Result<(), ServerTsTreeError> {
    let mut pending = vec![out_dir.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in
            fs::read_dir(&directory).map_err(|source| ServerTsTreeError::ReadDirectory {
                path: directory.clone(),
                source,
            })?
        {
            let entry = entry.map_err(|source| ServerTsTreeError::ReadDirectory {
                path: directory.clone(),
                source,
            })?;
            let path = entry.path();
            let file_type =
                entry
                    .file_type()
                    .map_err(|source| ServerTsTreeError::ReadFileType {
                        path: path.clone(),
                        source,
                    })?;
            if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.ends_with(".rs.d.ts") || name.ends_with(".rs.d.ts.map")
                    })
                && !expected_outputs.contains(&path)
            {
                fs::remove_file(&path)
                    .map_err(|source| ServerTsTreeError::RemoveFile { path, source })?;
            }
        }
    }
    Ok(())
}

fn write_generated_file(
    path: PathBuf,
    contents: impl AsRef<[u8]>,
) -> Result<(), ServerTsTreeError> {
    write_file(&path, contents)
        .map(|_| ())
        .map_err(|source| ServerTsTreeError::WriteFile { path, source })
}

#[derive(thiserror::Error, Diagnostic, Debug)]
pub enum ServerTsTreeError {
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
