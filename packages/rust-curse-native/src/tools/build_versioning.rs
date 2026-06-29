//! Manage generated build version output directories.

use std::{
    fs,
    path::{Path, PathBuf},
};

use miette::Diagnostic;
use thiserror::Error;

/// Returns a build-version-specific child directory under `path`.
pub fn get_versioned_dir(path: &Path, version: u64) -> PathBuf {
    path.join(version.to_string())
}

/// Removes numeric build-version directories except the current version.
pub fn cleanup_old_build_versions(
    output_root_path: &Path,
    current_version: u64,
) -> Result<(), CleanupOldBuildVersionsError> {
    for entry in fs::read_dir(output_root_path).map_err(|source| {
        CleanupOldBuildVersionsError::ReadOutputDirectory {
            path: output_root_path.to_path_buf(),
            source,
        }
    })? {
        let entry = entry.map_err(|source| CleanupOldBuildVersionsError::ReadOutputDirectory {
            path: output_root_path.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(version) = path
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .and_then(|file_name| file_name.parse::<u64>().ok())
        else {
            continue;
        };
        if version == current_version {
            continue;
        }
        fs::remove_dir_all(&path)
            .map_err(|source| CleanupOldBuildVersionsError::RemoveBuildVersion { path, source })?;
    }

    Ok(())
}

#[derive(Error, Diagnostic, Debug)]
pub enum CleanupOldBuildVersionsError {
    #[error("failed to read build output directory at {}", path.display())]
    #[diagnostic(code(rust_curse::build_versioning::read_output_directory))]
    ReadOutputDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to remove old build version at {}", path.display())]
    #[diagnostic(code(rust_curse::build_versioning::remove_build_version))]
    RemoveBuildVersion {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
