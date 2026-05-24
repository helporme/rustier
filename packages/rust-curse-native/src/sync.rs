use std::path::PathBuf;

use crate::sync::{write_build::BuildSwitch, write_lib::LibSwitch};

// todo: check how to remove pub and have tests
pub mod patch_cargo_toml;
pub mod tree;
pub mod write;
pub mod write_build;
pub mod write_lib;

pub mod write_napi;
pub mod write_orphan_tree;
pub mod write_rustdoc;
pub mod write_ts;
pub mod write_wasm;
pub mod write_wired_cargo_toml;

#[derive(Default, Debug)]
pub struct Syncer {
    /// Absolute path to the project root directory.
    pub project_root_abs_path: PathBuf,
    /// Output directory relative to the project root, usually `.rust-curse`.
    pub out_dir_rel_path: PathBuf,
    /// `lib.rs` content genereator.
    pub lib: LibSwitch,
    /// `build.rs` content generator.
    pub build: BuildSwitch,
}

impl Syncer {
    pub fn new(project_root_abs_path: PathBuf, out_dir_rel_path: PathBuf) -> Self {
        Self {
            project_root_abs_path,
            out_dir_rel_path,
            ..Default::default()
        }
    }

    /// Absolute path to the output directory
    /// (`project_root_abs_path / out_dir_rel_path`).
    pub fn abs_out_dir(&self) -> PathBuf {
        self.project_root_abs_path.join(&self.out_dir_rel_path)
    }
}
