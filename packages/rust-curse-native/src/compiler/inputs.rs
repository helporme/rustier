use std::{
    collections::{BTreeMap, HashMap},
    fs::read_to_string,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use rustdoc_types as doc;
use salsa::Setter;
use thiserror::Error;

use crate::compiler::db::Db;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum Api {
    /// wasm-bindgen
    Wasm { target: WasmTarget },
    /// napi
    Node,
}

#[allow(dead_code)]
impl Api {
    pub(super) fn set(db: &mut dyn Db, value: Self) {
        ApiInput::set(db, value);
    }

    pub(super) fn get(db: &dyn Db) -> Self {
        ApiInput::get(db).inner(db)
    }
}

/// JavaScript platform targeted by wasm-bindgen bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum WasmTarget {
    Web,
    Node,
}

#[salsa::input(singleton)]
struct ApiInput {
    #[returns(copy)]
    inner: Api,
}

#[allow(dead_code)]
impl ApiInput {
    fn set(db: &mut dyn Db, inner: Api) -> Self {
        if let Some(input) = Self::try_get(db) {
            input.set_inner(db).to(inner);
            input
        } else {
            Self::new(db, inner)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct CompilationMetadata {
    pub bin_path: PathBuf,
    pub js_ffi_path: PathBuf,
    pub js_mods_root_path: PathBuf,
}

impl CompilationMetadata {
    #[allow(dead_code)]
    pub(super) fn set(db: &mut dyn Db, value: Self) {
        CompilationMetadataInput::set(db, value);
    }

    pub(super) fn get(db: &dyn Db) -> Self {
        CompilationMetadataInput::get(db).inner(db)
    }
}

#[salsa::input(singleton)]
struct CompilationMetadataInput {
    pub inner: CompilationMetadata,
}

impl CompilationMetadataInput {
    #[allow(dead_code)]
    fn set(db: &mut dyn Db, inner: CompilationMetadata) -> Self {
        if let Some(input) = Self::try_get(db) {
            input.set_inner(db).to(inner);
            input
        } else {
            Self::new(db, inner)
        }
    }
}

#[salsa::input(singleton)]
pub(super) struct RustdocCrates {
    #[returns(ref)]
    pub(super) crates: BTreeMap<String, Crate>,
}

impl RustdocCrates {
    #[allow(dead_code)]
    pub(super) fn set(db: &mut dyn Db, crates: BTreeMap<String, Crate>) -> Self {
        if let Some(rustdoc) = Self::try_get(db) {
            rustdoc.set_crates(db).to(crates);
            rustdoc
        } else {
            Self::new(db, crates)
        }
    }

    pub fn crate_data<'db>(&self, db: &'db dyn Db, name: &str) -> Option<&'db Crate> {
        self.crates(db).get(name)
    }

    pub fn crate_<'db>(&self, db: &'db dyn Db, name: &str) -> Option<&'db doc::Crate> {
        self.crate_data(db, name).map(|crate_| &crate_.body)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Crate {
    pub body: doc::Crate,
    pub is_external: bool,
    pub dir: PathBuf,
}

#[salsa::input(singleton)]
pub(super) struct SourceFiles {
    #[no_eq]
    #[returns(clone)]
    cache: Arc<Mutex<HashMap<PathBuf, SourceFile>>>,
}

impl SourceFiles {
    pub(super) fn read(db: &dyn Db, path: &Path) -> Result<SourceFile, SourceFileLoadError> {
        let canonical_path =
            path.canonicalize()
                .map_err(|error| SourceFileLoadError::Canonicalize {
                    path: path.to_owned(),
                    message: error.to_string().into(),
                })?;

        let source_files = Self::try_get(db)
            .unwrap_or_else(|| Self::new(db, Arc::new(Mutex::new(HashMap::new()))));
        let cache = source_files.cache(db);
        let mut cache = cache
            .lock()
            .expect("source file cache lock must not be poisoned");

        if let Some(file) = cache.get(&canonical_path) {
            return Ok(*file);
        }

        let contents =
            read_to_string(&canonical_path).map_err(|error| SourceFileLoadError::Read {
                path: canonical_path.clone(),
                message: error.to_string().into(),
            })?;

        let file = SourceFile::new(db, canonical_path.clone(), Arc::from(contents));
        cache.insert(canonical_path, file);

        Ok(file)
    }
}

#[salsa::input]
pub(super) struct SourceFile {
    #[returns(ref)]
    pub(super) path: PathBuf,
    #[returns(clone)]
    pub(super) contents: Arc<str>,
}

#[derive(Error, Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub(super) enum SourceFileLoadError {
    #[error("failed to canonicalize source path `{path}`: {message}")]
    Canonicalize { path: PathBuf, message: Arc<str> },

    #[error("failed to read source file `{path}`: {message}")]
    Read { path: PathBuf, message: Arc<str> },
}
