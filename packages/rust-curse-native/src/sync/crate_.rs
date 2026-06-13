use std::path::PathBuf;

use crate::codegen;

/// Options for [`sync_crate`].
pub struct SyncCrateOps {
    /// The path to the Cargo.toml manifest file of a crate which should be
    /// synced.
    pub manifest_path: PathBuf,
    /// The path to the directory where the cursed crate will be crated.
    /// e.g. ".rust-curse/crates/"
    pub out_dir: PathBuf,
    /// The API to use for compiling bindings.
    pub api: codegen::bindings::Api,
    /// The database for compiling bindings.
    pub db: codegen::db::DbImpl,
}

/// Syncs a cursed crate for a given crate.
pub fn sync_crate(opts: SyncCrateOps) {
    todo!()
}
