use crate::{common::HEADER, orphans::OrphanModule};

use super::{tree, Scaffold, ScaffoldError, ScaffoldSection};

pub const RUSTDOC_ENV_VAR: &str = "RUST_CURSE_RUSTDOC";
pub const RUSTDOC_DIR: &str = "lib-rustdoc";

pub struct RustdocSectionOpts<'a> {
    pub modules: &'a [OrphanModule],
}

#[derive(Debug, thiserror::Error)]
pub enum RustdocSectionError {
    #[error(transparent)]
    Scaffold(#[from] ScaffoldError),
    #[error("rustdoc module '{name}' is both a leaf and has children")]
    LeafBranchCollision { name: String },
}

impl From<tree::TreeError> for RustdocSectionError {
    fn from(err: tree::TreeError) -> Self {
        match err {
            tree::TreeError::Scaffold(e) => Self::Scaffold(e),
            tree::TreeError::LeafBranchCollision { name } => Self::LeafBranchCollision { name },
        }
    }
}

impl Scaffold {
    pub fn set_rustdoc(&mut self, opts: RustdocSectionOpts) -> Result<(), RustdocSectionError> {
        let abs_external_dir = self.abs_path_to(RUSTDOC_DIR);
        let abs_project_root = self.abs_path_from_project_root_to("");

        let root = tree::build_tree(
            opts.modules
                .iter()
                .map(|m| (m.mod_name.as_str(), m.rel_rs_path.clone())),
        )?;

        let mut content = String::from(HEADER);
        tree::render_root_decls(&mut content, &root, &abs_project_root, &abs_external_dir);

        let section = ScaffoldSection::new("rustdoc")
            .exclusive_env_flag(RUSTDOC_ENV_VAR)
            .external(&format!("{RUSTDOC_DIR}/"))
            .content(content);
        self.set(section)?;

        tree::write_subtree(&root, &abs_project_root, &abs_external_dir, HEADER)?;

        Ok(())
    }
}
