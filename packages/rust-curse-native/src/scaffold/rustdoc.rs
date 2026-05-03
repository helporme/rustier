use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{common::HEADER, orphans::OrphanModule};

use super::{Scaffold, ScaffoldError, ScaffoldSection};

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

#[derive(Default)]
struct Node {
    leaf_rel_rs_path: Option<PathBuf>,
    children: BTreeMap<String, Node>,
}

impl Scaffold {
    pub fn set_rustdoc(&mut self, opts: RustdocSectionOpts) -> Result<(), RustdocSectionError> {
        let root = build_tree(opts.modules)?;

        let abs_rustdoc_dir = self.abs_path_to(RUSTDOC_DIR);
        let abs_project_root = self.abs_path_from_project_root_to("");

        let lib_body = render_lib(&root, &abs_project_root, &abs_rustdoc_dir);

        let section = ScaffoldSection::new("rustdoc")
            .env_flag(RUSTDOC_ENV_VAR)
            .external(&format!("{RUSTDOC_DIR}/"))
            .lib(lib_body);
        self.set(section)?;

        if abs_rustdoc_dir.exists() {
            fs::remove_dir_all(&abs_rustdoc_dir).map_err(|e| ScaffoldError::Io {
                path: abs_rustdoc_dir.clone(),
                source: e,
            })?;
        }

        let mut segs: Vec<String> = Vec::new();
        write_subtree(&root, &mut segs, &abs_project_root, &abs_rustdoc_dir)?;

        Ok(())
    }
}

fn build_tree(modules: &[OrphanModule]) -> Result<Node, RustdocSectionError> {
    let mut root = Node::default();

    for module in modules {
        let segs: Vec<&str> = module.mod_name.split("::").collect();
        let mut node = &mut root;
        for (i, seg) in segs.iter().enumerate() {
            let is_last = i == segs.len() - 1;
            let child = node.children.entry((*seg).to_string()).or_default();
            if is_last {
                if !child.children.is_empty() || child.leaf_rel_rs_path.is_some() {
                    return Err(RustdocSectionError::LeafBranchCollision {
                        name: module.mod_name.clone(),
                    });
                }
                child.leaf_rel_rs_path = Some(module.rel_rs_path.clone());
            } else if child.leaf_rel_rs_path.is_some() {
                return Err(RustdocSectionError::LeafBranchCollision {
                    name: segs[..=i].join("::"),
                });
            }
            node = child;
        }
    }

    Ok(root)
}

fn render_lib(root: &Node, abs_project_root: &Path, abs_rustdoc_dir: &Path) -> String {
    let mut out = String::from(HEADER);
    let mut segs: Vec<String> = Vec::new();
    for (seg, child) in &root.children {
        segs.push(seg.clone());
        out.push_str(&render_decl(
            seg,
            child,
            &segs,
            abs_project_root,
            abs_rustdoc_dir,
        ));
        segs.pop();
    }
    out
}

fn write_subtree(
    node: &Node,
    segs: &mut Vec<String>,
    abs_project_root: &Path,
    abs_rustdoc_dir: &Path,
) -> Result<(), ScaffoldError> {
    for (seg, child) in &node.children {
        segs.push(seg.clone());
        if !child.children.is_empty() {
            let mut body = String::from(HEADER);
            for (child_seg, grandchild) in &child.children {
                segs.push(child_seg.clone());
                body.push_str(&render_decl(
                    child_seg,
                    grandchild,
                    segs,
                    abs_project_root,
                    abs_rustdoc_dir,
                ));
                segs.pop();
            }

            let abs_file = abs_rustdoc_dir.join(format!("{}.rs", segs.join("/")));
            if let Some(parent) = abs_file.parent() {
                fs::create_dir_all(parent).map_err(|e| ScaffoldError::Io {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
            fs::write(&abs_file, body).map_err(|e| ScaffoldError::Io {
                path: abs_file,
                source: e,
            })?;

            write_subtree(child, segs, abs_project_root, abs_rustdoc_dir)?;
        }
        segs.pop();
    }
    Ok(())
}

fn render_decl(
    seg: &str,
    child: &Node,
    segs_to_child: &[String],
    abs_project_root: &Path,
    abs_rustdoc_dir: &Path,
) -> String {
    let target = if child.children.is_empty() {
        let leaf = child
            .leaf_rel_rs_path
            .as_ref()
            .expect("non-root leaf has rel_rs_path");
        abs_project_root.join(leaf)
    } else {
        abs_rustdoc_dir.join(format!("{}.rs", segs_to_child.join("/")))
    };

    let mut out = String::new();
    out.push_str(&format!("#[path = \"{}\"]\n", target.to_string_lossy()));
    if seg.chars().any(|c| c.is_ascii_uppercase()) {
        out.push_str("#[allow(non_snake_case)]\n");
    }
    out.push_str(&format!("pub mod {seg};\n\n"));
    out
}
