use std::{fs, io, path::Path};

use tracing::debug;

/// Writes `content` to `path` only when the file contents differ.
///
/// `Ok(true)` when the file was written, and `Ok(false)` when the
/// existing file already matched `content`.
#[tracing::instrument(level = "trace", skip(path, content), fields(path = %path.as_ref().display()))]
pub fn write_if_changed(path: impl AsRef<Path>, content: impl AsRef<[u8]>) -> io::Result<bool> {
    let path = path.as_ref();
    let content = content.as_ref();

    match fs::read(path) {
        Ok(existing) if existing == content => {
            debug!(bytes = content.len(), "file content unchanged");
            Ok(false)
        }
        Ok(_) => {
            debug!(bytes = content.len(), "rewriting changed file");
            fs::write(path, content)?;
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            debug!(bytes = content.len(), "writing missing file");
            fs::write(path, content)?;
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path, time::SystemTime};

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn writes_missing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("generated.rs");

        let wrote = write_if_changed(&path, "content").unwrap();

        assert!(wrote);
        assert_eq!(fs::read_to_string(path).unwrap(), "content");
    }

    #[test]
    fn skips_file_with_matching_contents() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("generated.rs");
        fs::write(&path, "content").unwrap();
        let modified_before = modified_at(&path);

        let wrote = write_if_changed(&path, "content").unwrap();

        assert!(!wrote);
        assert_eq!(modified_at(&path), modified_before);
    }

    #[test]
    fn overwrites_file_with_different_contents() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("generated.rs");
        fs::write(&path, "old content").unwrap();

        let wrote = write_if_changed(&path, "new content").unwrap();

        assert!(wrote);
        assert_eq!(fs::read_to_string(path).unwrap(), "new content");
    }

    fn modified_at(path: &Path) -> SystemTime {
        fs::metadata(path).unwrap().modified().unwrap()
    }
}
