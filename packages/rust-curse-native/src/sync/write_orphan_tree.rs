use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::codegen::asg::{Asg, AsgId, Item};

use super::tree::{self, TreeError};
use super::write_lib::{write_lib, LibBranch, WriteLibError};
use super::Syncer;

pub const ORPHAN_TREE_DIR: &str = "orphan-tree";

pub struct WriteOrphanTreeOpts<'a> {
    pub asg: &'a Asg,
    pub exposed: &'a HashSet<AsgId>,
}

/// Error returned by [`Syncer::write_orphan_tree`] when building the
/// module tree, rendering, or writing fails.
#[derive(Debug, thiserror::Error)]
pub enum WriteOrphanTreeError {
    #[error("failed to build module tree: {0}")]
    Tree(#[from] TreeError),
    #[error("failed to write lib.rs: {0}")]
    WriteLib(#[from] WriteLibError),
    #[error("failed to create output directory: {0}")]
    CreateOutDir(#[source] io::Error),
}

impl Syncer {
    /// Writes a `pub mod` tree of every local-crate module to `lib.rs`,
    /// plus `pub use` re-exports for every exposed export.
    ///
    /// `lib.rs` content is combined with previous `Syncer::write_*` calls if
    /// any.
    pub fn write_orphan_tree(
        &mut self,
        opts: WriteOrphanTreeOpts,
    ) -> Result<(), WriteOrphanTreeError> {
        let abs_out_dir = self.abs_out_dir();
        let abs_mod_out_dir = abs_out_dir.join(ORPHAN_TREE_DIR);
        let abs_project_root = self.project_root_abs_path.clone();

        fs::create_dir_all(&abs_out_dir).map_err(WriteOrphanTreeError::CreateOutDir)?;

        let mut local_modules: Vec<(String, PathBuf, AsgId)> = opts
            .asg
            .items
            .iter()
            .filter_map(|it| {
                let Item::Mod(m) = it else { return None };
                let id = m.id;
                if id == opts.asg.root {
                    return None;
                }
                let rel_rs_path = m.span.as_ref()?.filename.clone();
                let mod_name = m.path.segments[1..].join("::");
                Some((mod_name, rel_rs_path, id))
            })
            .collect();
        local_modules.sort_by(|a, b| a.0.cmp(&b.0));

        let root = tree::build_tree(
            local_modules
                .iter()
                .map(|(name, rel, _)| (name.as_str(), rel.clone())),
        )?;

        let mut content = String::new();
        tree::render_root_decls(
            &mut content,
            &root,
            &abs_project_root,
            &abs_mod_out_dir,
            "pub(crate)",
        );

        for (mod_name, _, id) in &local_modules {
            let mut names: Vec<&str> = opts
                .asg
                .get_mod_unchecked(*id)
                .exports
                .iter()
                .filter(|e| opts.exposed.contains(&e.target))
                .map(|e| e.name.as_str())
                .collect();
            names.sort_unstable();
            for name in names {
                content.push_str(&format!("pub use {mod_name}::{name};\n"));
            }
        }

        self.lib
            .set_branch("orphan_tree", LibBranch::new(content).include());
        write_lib(&abs_out_dir, &self.lib)?;

        tree::write_subtree(&root, &abs_project_root, &abs_mod_out_dir, "pub(crate)")?;

        Ok(())
    }
}
