use std::{
    fs, io,
    path::{Path, PathBuf},
};

use miette::Diagnostic;
use thiserror::Error;
use tracing::debug;

/// Writes `content` to `path` only when the file contents differ.
///
/// `Ok(true)` when the file was written, and `Ok(false)` when the
/// existing file already matched `content`.
pub fn write_file(
    path: impl AsRef<Path>,
    content: impl AsRef<[u8]>,
) -> Result<bool, WriteFileError> {
    let path = path.as_ref();
    let content = content.as_ref();

    match fs::read(path) {
        Ok(existing) if existing == content => {
            debug!(bytes = content.len(), "file content unchanged");
            return Ok(false);
        }
        Ok(_) => {
            debug!(bytes = content.len(), "rewriting changed file");
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            debug!(bytes = content.len(), "writing missing file");
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|source| WriteFileError::CreateDirectory {
                    path: parent.to_owned(),
                    source,
                })?;
            }
        }
        Err(source) => {
            return Err(WriteFileError::Read {
                path: path.to_owned(),
                source,
            });
        }
    }

    fs::write(path, content).map_err(|source| WriteFileError::Write {
        path: path.to_owned(),
        source,
    })?;
    Ok(true)
}

#[derive(Debug, Error, Diagnostic)]
pub enum WriteFileError {
    #[error("failed to read {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to create directory {}", path.display())]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write {}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path, time::SystemTime};

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn writes_missing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("generated.rs");

        let wrote = write_file(&path, "content").unwrap();

        assert!(wrote);
        assert_eq!(fs::read_to_string(path).unwrap(), "content");
    }

    #[test]
    fn skips_file_with_matching_contents() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("generated.rs");
        fs::write(&path, "content").unwrap();
        let modified_before = modified_at(&path);

        let wrote = write_file(&path, "content").unwrap();

        assert!(!wrote);
        assert_eq!(modified_at(&path), modified_before);
    }

    #[test]
    fn overwrites_file_with_different_contents() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("generated.rs");
        fs::write(&path, "old content").unwrap();

        let wrote = write_file(&path, "new content").unwrap();

        assert!(wrote);
        assert_eq!(fs::read_to_string(path).unwrap(), "new content");
    }

    fn modified_at(path: &Path) -> SystemTime {
        fs::metadata(path).unwrap().modified().unwrap()
    }
}
