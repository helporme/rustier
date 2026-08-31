use std::path::PathBuf;

use crate::{compiler::workspace::Workspace, server::db::Db};

/// Update of a file.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExternalUpdate {
    pub path: PathBuf,
    pub contents: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateReconcilerOpts {
    /// Path to the manfiest of the entry crate.
    pub manifest_path: String,
}

/// Reconciles an external file update into a
/// [compiler workspace](crate::compiler::workspace::Workspace).
///
/// Updates file content, crate graph and other realted inputs.
#[derive(Debug, Clone)]
pub struct UpdateReconciler {
    opts: UpdateReconcilerOpts,
    // stores crates versions map, once changes rescan is fired per crate
}

impl UpdateReconciler {
    pub fn new(opts: UpdateReconcilerOpts) -> Self {
        Self { opts }
    }

    /// Reconciles a change into workspace.
    pub fn reconcile(&self, db: &mut dyn Db, workspace: &mut Workspace, update: &ExternalUpdate) {
        todo!()
    }
}
