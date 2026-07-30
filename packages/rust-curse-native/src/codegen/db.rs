use std::{
    collections::HashMap,
    fs::read_to_string,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use thiserror::Error;

/// A source file loaded on demand for diagnostic rendering.
#[salsa::input]
pub struct SourceFile {
    #[returns(ref)]
    pub path: PathBuf,
    #[returns(clone)]
    pub contents: Arc<str>,
}

/// Concrete code generation database with an on-demand source-file cache.
#[salsa::db]
#[derive(Clone, Default)]
pub struct DbImpl {
    storage: salsa::Storage<Self>,
    source_files: Arc<Mutex<HashMap<PathBuf, SourceFile>>>,
}

#[salsa::db]
impl salsa::Database for DbImpl {}

/// Salsa database used by code generation queries.
#[salsa::db]
pub trait Db: salsa::Database {
    /// Returns the canonical source file input for `path`, loading it on first
    /// use.
    fn source_file(&self, path: &Path) -> Result<SourceFile, SourceFileLoadError>;
}

/// Error returned when a diagnostic source file cannot be loaded.
#[derive(Error, Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SourceFileLoadError {
    #[error("failed to canonicalize source path `{path}`: {message}")]
    Canonicalize { path: PathBuf, message: Arc<str> },

    #[error("failed to read source file `{path}`: {message}")]
    Read { path: PathBuf, message: Arc<str> },
}

#[salsa::db]
impl Db for DbImpl {
    fn source_file(&self, path: &Path) -> Result<SourceFile, SourceFileLoadError> {
        let canonical_path =
            path.canonicalize()
                .map_err(|error| SourceFileLoadError::Canonicalize {
                    path: path.to_owned(),
                    message: error.to_string().into(),
                })?;

        let mut source_files = self
            .source_files
            .lock()
            .expect("source file cache lock must not be poisoned");

        if let Some(file) = source_files.get(&canonical_path) {
            return Ok(*file);
        }

        let contents =
            read_to_string(&canonical_path).map_err(|error| SourceFileLoadError::Read {
                path: canonical_path.clone(),
                message: error.to_string().into(),
            })?;

        let file = SourceFile::new(self, canonical_path.clone(), Arc::from(contents));
        source_files.insert(canonical_path, file);

        Ok(file)
    }
}
