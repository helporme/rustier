use std::{
    fs,
    path::{Path, PathBuf},
};

use miette::Diagnostic;
use thiserror::Error;

pub fn versioned_dir(path: &Path, version: u32) -> PathBuf {
    path.join(version.to_string())
}

pub fn cleanup(output_root: &Path, current_version: u32) -> Result<(), BuildVersioningError> {
    for entry in
        fs::read_dir(output_root).map_err(|source| BuildVersioningError::ReadOutputDirectory {
            path: output_root.to_path_buf(),
            source,
        })?
    {
        let entry = entry.map_err(|source| BuildVersioningError::ReadOutputDirectory {
            path: output_root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let is_old_version = path.is_dir()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.parse::<u32>().ok())
                .is_some_and(|version| version != current_version);
        if is_old_version {
            fs::remove_dir_all(&path)
                .map_err(|source| BuildVersioningError::RemoveBuildVersion { path, source })?;
        }
    }
    Ok(())
}

#[derive(Debug, Diagnostic, Error)]
pub enum BuildVersioningError {
    #[error("failed to read build output directory at {}", path.display())]
    ReadOutputDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to remove old build version at {}", path.display())]
    RemoveBuildVersion {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
