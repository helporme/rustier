//! Path used inside the compiler to hide the VfsPath.

use std::path::Path;

use ra_ap_ide_db::base_db::{LibraryRoots, LocalRoots};
use ra_ap_vfs::VfsPath;
use thiserror::Error;
use triomphe::Arc;

use crate::compiler::db::Db;

/// Path to a file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct FilePath(FilePathKind);

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
enum FilePathKind {
    /// Path to the in-memory file. Must start with `/`.
    Virtual(String),
    /// Absolute path to the file stored inside file system.
    Real(String),
}

impl FilePath {
    /// Creates a path to an in-memory file.
    pub fn new_virtual(path: impl Into<String>) -> Result<Self, FilePathError> {
        let path = path.into();
        if !path.starts_with('/') {
            return Err(FilePathError::InvalidVirtualPath { path });
        }
        Ok(Self(FilePathKind::Virtual(path)))
    }

    /// Creates a path to a file on the real filesystem.
    pub fn new_real(path: impl Into<String>) -> Result<Self, FilePathError> {
        let path = path.into();
        if !Path::new(&path).is_absolute() {
            return Err(FilePathError::RelativeRealPath { path });
        }
        Ok(Self(FilePathKind::Real(path)))
    }

    pub fn as_path(&self) -> &Path {
        match &self.0 {
            FilePathKind::Virtual(path) | FilePathKind::Real(path) => Path::new(path),
        }
    }

    pub(super) fn contents(&self, db: &dyn Db) -> Option<Arc<str>> {
        let vfs_path = self.clone().into();
        let local_roots = LocalRoots::try_get(db)
            .into_iter()
            .flat_map(|roots| roots.roots(db).iter().copied());

        let library_roots = LibraryRoots::try_get(db)
            .into_iter()
            .flat_map(|roots| roots.roots(db).iter().copied());

        let file_id = local_roots.chain(library_roots).find_map(|root_id| {
            db.source_root(root_id)
                .source_root(db)
                .file_for_path(&vfs_path)
                .copied()
        })?;

        Some(db.file_text(file_id).text(db).clone())
    }
}

impl From<VfsPath> for FilePath {
    fn from(value: VfsPath) -> Self {
        match value.as_path() {
            Some(path) => {
                Self::new_real(path.to_string()).expect("rust-analyzer real paths must be absolute")
            }
            None => Self::new_virtual(value.to_string())
                .expect("rust-analyzer virtual paths must start with `/`"),
        }
    }
}

impl From<FilePath> for VfsPath {
    fn from(value: FilePath) -> Self {
        match value.0 {
            FilePathKind::Virtual(path) => Self::new_virtual_path(path),
            FilePathKind::Real(path) => Self::new_real_path(path),
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FilePathError {
    #[error("virtual file path must start with `/`: {path}")]
    InvalidVirtualPath { path: String },

    #[error("real file path must be absolute: {path}")]
    RelativeRealPath { path: String },
}
