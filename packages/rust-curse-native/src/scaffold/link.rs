//! Always-applied "link" section that materialises a `pub mod` tree of
//! exposed orphan modules.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::common::HEADER;
use crate::ir::{Id, Item, IR};
use crate::orphans::OrphanModule;

use super::{tree, Scaffold, ScaffoldError, ScaffoldSection};

pub const LINK_DIR: &str = "lib-link";

pub struct LinkSectionOpts<'a> {
    pub modules: &'a [OrphanModule],
    pub ir: &'a IR,
    pub exposed: &'a HashSet<Id>,
}

#[derive(Debug, thiserror::Error)]
pub enum LinkSectionError {
    #[error(transparent)]
    Scaffold(#[from] ScaffoldError),
    #[error("link module '{name}' is both a leaf and has children")]
    LeafBranchCollision { name: String },
}

impl From<tree::TreeError> for LinkSectionError {
    fn from(err: tree::TreeError) -> Self {
        match err {
            tree::TreeError::Scaffold(e) => Self::Scaffold(e),
            tree::TreeError::LeafBranchCollision { name } => Self::LeafBranchCollision { name },
        }
    }
}

impl Scaffold {
    pub fn set_link(&mut self, opts: LinkSectionOpts) -> Result<(), LinkSectionError> {
        let abs_external_dir = self.abs_path_to(LINK_DIR);
        let abs_project_root = self.abs_path_from_project_root_to("");

        let exposed_modules: Vec<(&str, PathBuf)> = opts
            .modules
            .iter()
            .filter(|m| module_has_exposed_export(opts.ir, &m.mod_name, opts.exposed))
            .map(|m| (m.mod_name.as_str(), m.rel_rs_path.clone()))
            .collect();

        let root = tree::build_tree(exposed_modules.iter().map(|(n, p)| (*n, p.clone())))?;

        let mut content = String::from(HEADER);
        tree::render_root_decls(&mut content, &root, &abs_project_root, &abs_external_dir);

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

        let section = ScaffoldSection::new("link")
            .external(&format!("{LINK_DIR}/"))
            .content(content);
        self.set(section)?;

        tree::write_subtree(&root, &abs_project_root, &abs_external_dir, HEADER)?;

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
