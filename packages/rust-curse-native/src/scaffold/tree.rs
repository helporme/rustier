//! Shared file module-tree builder used by the `link` and `rustdoc` sections.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::ScaffoldError;

#[derive(Default, Debug)]
pub(super) struct Node {
    pub leaf_rel_rs_path: Option<PathBuf>,
    pub children: BTreeMap<String, Node>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum TreeError {
    #[error(transparent)]
    Scaffold(#[from] ScaffoldError),
    #[error("module '{name}' is both a leaf and has children")]
    LeafBranchCollision { name: String },
}

pub(super) fn build_tree<I, S, P>(entries: I) -> Result<Node, TreeError>
where
    I: IntoIterator<Item = (S, P)>,
    S: AsRef<str>,
    P: Into<PathBuf>,
{
    let mut root = Node::default();
    for (mod_name, rel_rs_path) in entries {
        let mod_name = mod_name.as_ref();
        let segs: Vec<&str> = mod_name.split("::").collect();
        let mut node = &mut root;
        for (i, seg) in segs.iter().enumerate() {
            let is_last = i == segs.len() - 1;
            let child = node.children.entry((*seg).to_string()).or_default();
            if is_last {
                if !child.children.is_empty() || child.leaf_rel_rs_path.is_some() {
                    return Err(TreeError::LeafBranchCollision {
                        name: mod_name.to_string(),
                    });
                }
                child.leaf_rel_rs_path = Some(rel_rs_path.into());
                break;
            } else if child.leaf_rel_rs_path.is_some() {
                return Err(TreeError::LeafBranchCollision {
                    name: segs[..=i].join("::"),
                });
            }
            node = child;
        }
    }
    Ok(root)
}

/// Append `pub mod <seg>;` declarations for the root node's direct
/// children to `out`.
pub(super) fn render_root_decls(
    out: &mut String,
    root: &Node,
    abs_project_root: &Path,
    abs_external_dir: &Path,
) {
    let mut segs: Vec<String> = Vec::new();
    for (seg, child) in &root.children {
        segs.push(seg.clone());
        out.push_str(&render_decl(
            seg,
            child,
            &segs,
            abs_project_root,
            abs_external_dir,
        ));
        segs.pop();
    }
}

/// Wipe `abs_external_dir` if it exists, then write a `.rs` file for
/// every non-leaf node beneath `root`. Each file contains `header` plus
/// `pub mod` declarations for that node's children.
pub(super) fn write_subtree(
    root: &Node,
    abs_project_root: &Path,
    abs_external_dir: &Path,
    header: &str,
) -> Result<(), ScaffoldError> {
    if abs_external_dir.exists() {
        fs::remove_dir_all(abs_external_dir).map_err(|e| ScaffoldError::Io {
            path: abs_external_dir.to_path_buf(),
            source: e,
        })?;
    }
    let mut segs: Vec<String> = Vec::new();
    write_subtree_inner(root, &mut segs, abs_project_root, abs_external_dir, header)
}

fn write_subtree_inner(
    node: &Node,
    segs: &mut Vec<String>,
    abs_project_root: &Path,
    abs_external_dir: &Path,
    header: &str,
) -> Result<(), ScaffoldError> {
    for (seg, child) in &node.children {
        segs.push(seg.clone());
        if !child.children.is_empty() {
            let mut body = String::from(header);
            for (child_seg, grandchild) in &child.children {
                segs.push(child_seg.clone());
                body.push_str(&render_decl(
                    child_seg,
                    grandchild,
                    segs,
                    abs_project_root,
                    abs_external_dir,
                ));
                segs.pop();
            }

            let abs_file = abs_external_dir.join(format!("{}.rs", segs.join("/")));
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

            write_subtree_inner(child, segs, abs_project_root, abs_external_dir, header)?;
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
    abs_external_dir: &Path,
) -> String {
    let target = if child.children.is_empty() {
        let leaf = child
            .leaf_rel_rs_path
            .as_ref()
            .expect("non-root leaf has rel_rs_path");
        abs_project_root.join(leaf)
    } else {
        abs_external_dir.join(format!("{}.rs", segs_to_child.join("/")))
    };

    let mut out = String::new();
    out.push_str(&format!("#[path = \"{}\"]\n", target.to_string_lossy()));
    if seg.chars().any(|c| c.is_ascii_uppercase()) {
        out.push_str("#[allow(non_snake_case)]\n");
    }
    out.push_str(&format!("pub mod {seg};\n\n"));
    out
}
