//! Filesystem state used by the server.

mod load;

use crate::compiler::{FilePath, WorkspaceChange};

pub use load::{FsLoadError, ServerFsLoadOpts};

/// Tracks files watched by the server and pending compiler input changes.
pub struct ServerFs {
    pending_change: WorkspaceChange,
    active_files: Vec<String>,
}

impl ServerFs {
    /// Records local file changes made since the last compiler update.
    pub fn update(&mut self, changes: impl IntoIterator<Item = (FilePath, Option<String>)>) {
        for (path, contents) in changes {
            let active_path = path.as_path().to_string_lossy().into_owned();
            if contents.is_some() {
                if !self.active_files.contains(&active_path) {
                    self.active_files.push(active_path);
                    self.active_files.sort();
                }
            } else {
                self.active_files.retain(|path| path != &active_path);
            }
            self.pending_change.set_local_file(path, contents);
        }
    }

    /// Takes all compiler changes accumulated since the previous call.
    pub fn take_workspace_change(&mut self) -> WorkspaceChange {
        std::mem::replace(&mut self.pending_change, WorkspaceChange::new())
    }

    /// Returns local project files which should be watched for changes.
    pub fn active_files(&self) -> &Vec<String> {
        &self.active_files
    }
}
