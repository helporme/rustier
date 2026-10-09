//! Path used inside the compiler to hide the VfsPath.

use std::path::{Path, PathBuf};

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

    /// The path as a [`Path`], irrespective of whether it is virtual or real.
    pub fn as_path(&self) -> &Path {
        match &self.0 {
            FilePathKind::Virtual(path) | FilePathKind::Real(path) => Path::new(path),
        }
    }

    pub fn as_str(&self) -> &str {
        match &self.0 {
            FilePathKind::Virtual(path) | FilePathKind::Real(path) => path.as_str(),
        }
    }

    /// `true` if the path is a real file on the filesystem.
    pub fn is_real(&self) -> bool {
        matches!(self.0, FilePathKind::Real(_))
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

    pub(super) fn relative_path(&self, dst: &FilePath) -> Option<String> {
        let from_mod_path = self.as_path();
        let to_mod_path = dst.as_path();
        let mut from_components = from_mod_path.parent()?.components().peekable();
        let mut to_components = to_mod_path.components().peekable();

        while matches!(
            (from_components.peek(), to_components.peek()),
            (Some(from), Some(to)) if from == to
        ) {
            from_components.next();
            to_components.next();
        }

        if from_components.clone().any(|component| {
            matches!(
                component,
                std::path::Component::Prefix(_) | std::path::Component::RootDir
            )
        }) || to_components.clone().any(|component| {
            matches!(
                component,
                std::path::Component::Prefix(_) | std::path::Component::RootDir
            )
        }) {
            return None;
        }

        let mut path = String::new();
        let parent_count = from_components.count();
        if parent_count == 0 {
            path.push_str("./");
        } else {
            for _ in 0..parent_count {
                path.push_str("../");
            }
        }

        for (index, component) in to_components.enumerate() {
            if index > 0 {
                path.push('/');
            }
            path.push_str(&component.as_os_str().to_string_lossy());
        }

        Some(path)
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

impl TryFrom<&Path> for FilePath {
    type Error = FilePathError;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let path = path
            .to_str()
            .ok_or_else(|| FilePathError::NonUtf8RealPath {
                path: path.to_owned(),
            })?;
        Self::new_real(path)
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FilePathError {
    #[error("virtual file path must start with `/`: {path}")]
    InvalidVirtualPath { path: String },

    #[error("real file path must be absolute: {path}")]
    RelativeRealPath { path: String },

    #[error("real file path is not valid UTF-8: {}", path.display())]
    NonUtf8RealPath { path: PathBuf },
}
