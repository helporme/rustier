use std::path::PathBuf;

/// An orphan .rs module stored alongside .js code.
// `item_ids` is populated later in Step 3 (link) after rustdoc runs.
#[derive(Clone, Debug)]
pub struct OrphanModule {
    /// Module name using `::` separators, e.g. `foo::bar`.
    pub name: String,
    /// Path relative to the module_dir root (e.g. `greet.rs`, `nested/deep.rs`).
    pub rel_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct GeneratedFile {
    /// Relative path (e.g. `lib.rs`, `modules/math.rs`). The emit layer
    /// joins this with the output directory.
    pub rel_path: PathBuf,
    pub content: String,
}
