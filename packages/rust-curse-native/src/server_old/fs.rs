//! Filesystem state used by the server.

mod load;

use std::{
    collections::{BTreeSet, HashSet},
    path::PathBuf,
};

use crate::compiler::{FilePath, WorkspaceChange};

use glob::Pattern;
pub use load::FsReloadError;

/// Options for [`ServerFs::load`].
#[derive(Debug, Clone)]
pub struct ServerFsOpts {
    /// Path to the manifest of the root package.
    pub manifest_path: PathBuf,
    /// Glob patterns for files loaded into the compiler.
    pub included: Vec<Pattern>,
    /// Glob patterns for files and directories to skip.
    pub excluded: Vec<Pattern>,
    /// Additional dependency package or crate names to load.
    /// Every resolved version with a matching name is included.
    pub include_dep_crates: HashSet<String>,
    /// Dependency package or crate names omitted from the compiler.
    /// Every resolved version with a matching name is excluded.
    pub exclude_dep_crates: HashSet<String>,
    /// Whether standard-library crates should be loaded from the Rust sysroot.
    pub include_std: bool,
}

impl ServerFsOpts {
    /// Creates load options with Rust source and common build-directory
    /// filters.
    pub fn new(manifest_path: PathBuf) -> Self {
        Self {
            manifest_path,
            included: vec![Pattern::new("*.rs").unwrap()],
            excluded: vec![
                Pattern::new(".git").unwrap(),
                Pattern::new(".hg").unwrap(),
                Pattern::new(".svn").unwrap(),
                Pattern::new("node_modules").unwrap(),
                Pattern::new("target").unwrap(),
            ],
            include_dep_crates: HashSet::new(),
            exclude_dep_crates: HashSet::new(),
            include_std: true,
        }
    }
}

/// Tracks files watched by the server and pending compiler input changes.
pub struct ServerFs {
    pending_change: WorkspaceChange,
    active_files: BTreeSet<String>,
    opts: ServerFsOpts,
}

impl ServerFs {
    pub fn new(opts: ServerFsOpts) -> Self {
        Self {
            opts,
            pending_change: WorkspaceChange::new(),
            active_files: BTreeSet::new(),
        }
    }

    /// Realod filesystem state and compiler inputs from a Cargo project.
    pub fn reload(&mut self) -> Result<(), FsReloadError> {
        self.active_files.clear();
        self.pending_change = WorkspaceChange::new();
        self.load_project()
    }

    /// Records local file changes made since the last compiler update.
    pub fn update(
        &mut self,
        changes: impl IntoIterator<Item = (FilePath, Option<String>)>,
    ) -> Result<(), FsReloadError> {
        let changes = changes.into_iter().collect::<Vec<_>>();
        if changes.iter().any(|(path, _)| {
            path.as_path()
                .file_name()
                .is_some_and(|name| name == "Cargo.toml" || name == "Cargo.lock")
        }) {
            return self.reload();
        }

        for (path, contents) in changes {
            let active_path = path.as_path().to_string_lossy().into_owned();
            if contents.is_some() {
                self.active_files.insert(active_path);
            } else {
                self.active_files.remove(&active_path);
            }

            self.pending_change.set_local_file(path, contents);
        }
        Ok(())
    }

    /// Takes all compiler changes accumulated since the previous call.
    pub fn take_workspace_change(&mut self) -> WorkspaceChange {
        std::mem::replace(&mut self.pending_change, WorkspaceChange::new())
    }

    /// Returns local project files which should be watched for changes.
    pub fn active_files(&self) -> &BTreeSet<String> {
        &self.active_files
    }
}
