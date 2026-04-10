use std::path::PathBuf;

/// An orphan .rs module stored alongside .js code.
#[derive(Clone, Debug)]
pub struct OrphanModule {
    /// Module name using `::` separators, e.g. `foo::bar`.
    pub name: String,
    /// Path relative to the module_dir root (e.g. `greet.rs`, `nested/deep.rs`).
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
