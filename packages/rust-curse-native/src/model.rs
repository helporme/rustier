use std::path::PathBuf;

/// An orphan .rs module stored alongside .js code.
#[derive(Clone, Debug)]
pub struct OrphanModule {
    /// Module name using `::` separators, e.g. `foo::bar`.
    pub name: String,
    /// Path relative to the module_dir root (e.g. `greet.rs`,
    /// `nested/deep.rs`).
    pub rel_path: PathBuf,
    /// Rustdoc item IDs whose span.filename matches this module's source file.
    pub item_ids: Vec<rustdoc_types::Id>,
}

#[derive(Clone, Debug)]
pub struct GeneratedFile {
    /// Relative path (e.g. `lib.rs`, `modules/math.rs`). The emit layer
    /// joins this with the output directory.
    pub rel_path: PathBuf,
    pub content: String,
}

/// A single ESM import of a Rust module found in a JS/TS file.
#[derive(Clone, Debug)]
pub struct RsImport {
    /// Absolute path to the JS/TS file containing the import.
    pub js_abs_path: PathBuf,
    /// Relative path of the imported Rust module (matches
    /// `OrphanModule.rel_path`).
    pub rs_rel_path: PathBuf,
    /// Unresolved imported member names (may include "*")
    pub members: Vec<String>,
}

/// An OrphanModule that survived treeshaking
#[derive(Clone, Debug)]
pub struct ShakedModule {
    pub module: OrphanModule,
    /// Absolute paths of JS/TS files that import this module.
    pub importer_abs_paths: Vec<PathBuf>,
    /// Rustdoc item IDs of the members actually imported.
    pub imported_item_ids: Vec<rustdoc_types::Id>,
}
