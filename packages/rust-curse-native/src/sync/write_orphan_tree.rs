use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::ir::{Id, Item, IR};
use crate::orphans::OrphanModule;

use super::tree::{self, TreeError};
use super::write_lib::{write_lib, LibBranch, WriteLibError};
use super::Syncer;

pub const ORPHAN_TREE_DIR: &str = "orphan-tree";

pub struct WriteOrphanTreeOpts<'a> {
    pub modules: &'a [OrphanModule],
    pub ir: &'a IR,
    pub exposed: &'a HashSet<Id>,
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
    /// Writes a `pub mod` tree of every orphan module that exposes at
    /// least one exported item to `lib.rs`, plus `pub use` re-exports
    /// for every exposed export.
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

        let exposed_modules: Vec<(&str, PathBuf)> = opts
            .modules
            .iter()
            .filter(|m| module_has_exposed_export(opts.ir, &m.mod_name, opts.exposed))
            .map(|m| (m.mod_name.as_str(), m.rel_rs_path.clone()))
            .collect();

        let root = tree::build_tree(exposed_modules.iter().map(|(n, p)| (*n, p.clone())))?;

        let mut content = String::new();
        tree::render_root_decls(&mut content, &root, &abs_project_root, &abs_mod_out_dir);

        for module in opts.modules {
            let Some(id) = find_module(opts.ir, &module.mod_name) else {
                continue;
            };
            let mut names: Vec<&str> = opts
                .ir
                .get_mod_unchecked(id)
                .exports
                .iter()
                .filter(|e| opts.exposed.contains(&e.target))
                .map(|e| e.name.as_str())
                .collect();
            names.sort_unstable();
            for name in names {
                content.push_str(&format!("pub use {}::{name};\n", module.mod_name));
            }
        }

        self.lib.set_branch("orphan_tree", LibBranch::new(content));
        write_lib(&abs_out_dir, &self.lib)?;

        tree::write_subtree(&root, &abs_project_root, &abs_mod_out_dir)?;

        Ok(())
    }
}

fn module_has_exposed_export(ir: &IR, mod_name: &str, exposed: &HashSet<Id>) -> bool {
    let Some(id) = find_module(ir, mod_name) else {
        return false;
    };
    ir.get_mod_unchecked(id)
        .exports
        .iter()
        .any(|e| exposed.contains(&e.target))
}

fn find_module(ir: &IR, mod_name: &str) -> Option<Id> {
    let expected_tail: Vec<&str> = mod_name.split("::").collect();
    for (i, item) in ir.items.iter().enumerate() {
        let Item::Mod(def) = item else { continue };
        let segs: Vec<&str> = def.path.segments.iter().map(String::as_str).collect();
        if segs.first().copied() != Some(ir.crate_name.as_str()) {
            continue;
        }
        if segs.len() == expected_tail.len() + 1 && segs[1..] == expected_tail[..] {
            return Some(Id(i as u32));
        }
    }
    None
}
