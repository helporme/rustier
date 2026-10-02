use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use crate::compiler::path::{FilePath, FilePathError};

/// Reserve a fresh output path so watch-mode builds bypass runtime caches.
pub(super) fn reserve_unique_artifact_dir(out_dir: &Path) -> Result<PathBuf, ArtifactError> {
    let root = out_dir.join("artifacts");
    let io_error = |operation| {
        let path = root.clone();
        move |source| ArtifactError::Io {
            operation,
            path,
            source,
        }
    };
    fs::create_dir_all(&root).map_err(io_error("create artifact directory"))?;
    let mut version = 0_u64;
    for entry in fs::read_dir(&root).map_err(io_error("read artifact directory"))? {
        let entry = entry.map_err(io_error("read artifact entry"))?;
        if let Some(existing) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u64>().ok())
        {
            version = version.max(
                existing
                    .checked_add(1)
                    .ok_or(ArtifactError::VersionOverflow)?,
            );
        }
    }

    loop {
        let dir = root.join(version.to_string());
        match fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                version = version
                    .checked_add(1)
                    .ok_or(ArtifactError::VersionOverflow)?;
            }
            Err(source) => {
                return Err(ArtifactError::Io {
                    operation: "reserve artifact directory",
                    path: dir,
                    source,
                });
            }
        }
    }
}

pub(super) fn copy_artifact(source: &Path, destination: &Path) -> Result<FilePath, ArtifactError> {
    fs::copy(source, destination).map_err(|source| ArtifactError::Io {
        operation: "copy artifact to",
        path: destination.to_owned(),
        source,
    })?;
    Ok(destination.try_into()?)
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum ArtifactError {
    #[error("invalid build artifact path: {0}")]
    #[diagnostic(code(rust_curse::invalid_artifact_path))]
    Path(#[from] FilePathError),
    #[error("failed to {operation} {}", path.display())]
    #[diagnostic(code(rust_curse::artifact_io))]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("artifact version overflow")]
    #[diagnostic(code(rust_curse::artifact_version_overflow))]
    VersionOverflow,
}
