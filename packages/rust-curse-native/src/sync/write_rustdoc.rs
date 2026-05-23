use std::fs;
use std::io;

use crate::orphans::OrphanModule;

use super::tree::{self, TreeError};
use super::write_build::{env_flag_build_content, write_build, BuildBranch, WriteBuildError};
use super::write_lib::{write_lib, LibBranch, WriteLibError};
use super::Syncer;

pub const RUSTDOC_ENV_VAR: &str = "RUST_CURSE_RUSTDOC";
pub const RUSTDOC_CFG_FLAG: &str = "rust_curse_rustdoc";
pub const RUSTDOC_DIR: &str = "rustdoc";

/// Error returned by [`Syncer::write_rustdoc`] when building the module
/// tree, rendering, or writing fails.
#[derive(Debug, thiserror::Error)]
pub enum WriteRustdocError {
    #[error("failed to build module tree: {0}")]
    Tree(#[from] TreeError),
    #[error("failed to write lib.rs: {0}")]
    WriteLib(#[from] WriteLibError),
    #[error("failed to write build.rs: {0}")]
    WriteBuild(#[from] WriteBuildError),
    #[error("failed to create output directory: {0}")]
    CreateOutDir(#[source] io::Error),
}

impl Syncer {
    /// Writes a `pub mod` tree of every orphan module to `lib.rs` and
    /// `build.rs`, gated behind the `RUST_CURSE_RUSTDOC` env flag.
    ///
    /// `lib.rs` and `build.rs` content is combined with previous
    /// `Syncer::write_*` calls if any.
    pub fn write_rustdoc(&mut self, modules: &[OrphanModule]) -> Result<(), WriteRustdocError> {
        let abs_out_dir = self.abs_out_dir();
        let abs_mod_out_dir = abs_out_dir.join(RUSTDOC_DIR);
        let abs_project_root = self.project_root_abs_path.clone();

        fs::create_dir_all(&abs_out_dir).map_err(WriteRustdocError::CreateOutDir)?;

        let root = tree::build_tree(
            modules
                .iter()
                .map(|m| (m.mod_name.as_str(), m.rel_rs_path.clone())),
        )?;

        let mut content = String::new();
        tree::render_root_decls(
            &mut content,
            &root,
            &abs_project_root,
            &abs_mod_out_dir,
            "pub",
        );

        self.lib.set_xor_branch(
            "rustdoc",
            LibBranch::new(content)
                .requires_env_flag(RUSTDOC_CFG_FLAG)
                .include(),
        );
        write_lib(&abs_out_dir, &self.lib)?;

        self.build.set_branch(
            "rustdoc",
            BuildBranch::new(env_flag_build_content(RUSTDOC_ENV_VAR)),
        );
        write_build(&abs_out_dir, &self.build)?;

        tree::write_subtree(&root, &abs_project_root, &abs_mod_out_dir, "pub")?;

        Ok(())
    }
}
