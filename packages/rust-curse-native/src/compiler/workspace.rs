use std::{collections::HashMap, path::Path};

use ra_ap_ide_db::ChangeWithProcMacros;
use ra_ap_load_cargo::{LoadCargoConfig, load_workspace_at};
use ra_ap_project_model::CargoConfig;
use ra_ap_vfs::{Change, Vfs};

use crate::compiler::{
    inputs::{FilePaths, Platform, WasmEnv::Web},
    path::FilePath,
};

use super::db::RootDatabase;

/// Workspace exposing convenient methods to set up an environment for a
/// rust-analyzer database and access it via [`Self::db`].
///
/// Useful for testing and quick project bootstrapping. Alternatively, you can
/// use the rust-analyzer [`RootDatabase`] directly and manually set
/// rust-curse-specific [`inputs`](super::inputs).
#[derive(Debug)]
pub struct Workspace {
    db: RootDatabase,
    vfs: Vfs,
}

impl Workspace {
    /// Loads a workspace from the given path using the specified cargo
    /// configuration.
    pub fn load(
        root: &Path,
        cargo_config: &CargoConfig,
        load_config: &LoadCargoConfig,
    ) -> anyhow::Result<Self> {
        let (mut db, vfs, _) = load_workspace_at(root, cargo_config, load_config, &|msg| {
            tracing::info!("{msg}")
        })?;

        Platform::set(&mut db, Platform::Wasm { target: Web });

        FilePaths::set(
            &mut db,
            FilePaths {
                ffi_js_file: FilePath::new_virtual("/@ffi").unwrap(),
            },
        );

        Ok(Self { db, vfs })
    }

    /// Applies file changes to the workspace database.
    pub fn apply_change(&mut self, change: WorkspaceChange) {
        for (path, contents) in change.files {
            self.vfs
                .set_file_contents(path.into(), contents.map(String::into_bytes));
        }

        let mut analysis_change = ChangeWithProcMacros::default();
        let mut changed = false;

        for (_, file) in self.vfs.take_changes() {
            match file.change {
                Change::Create(contents, _) | Change::Modify(contents, _) => {
                    if let Ok(text) = String::from_utf8(contents) {
                        analysis_change.change_file(file.file_id, Some(text));
                        changed = true;
                    }
                }
                Change::Delete => {
                    analysis_change.change_file(file.file_id, None);
                    changed = true;
                }
            }
        }

        if changed {
            self.db.apply_change(analysis_change);
        }
    }

    /// Changes the compilation target platform.
    pub fn change_platform(&mut self, platform: Platform) {
        Platform::set(&mut self.db, platform);
    }

    /// Sets a custom FFI layout.
    pub fn change_ffi_layout(&mut self, ffi_layout: FilePaths) {
        FilePaths::set(&mut self.db, ffi_layout);
    }

    /// The database used for analysis and compilation queries.
    pub fn db(&self) -> &RootDatabase {
        &self.db
    }
}

/// File-only changes to apply to a [`Workspace`].
#[derive(Debug, Default)]
pub struct WorkspaceChange {
    files: HashMap<FilePath, Option<String>>,
}

impl WorkspaceChange {
    /// Records a file creation, update, or deletion.
    pub fn set_file(&mut self, path: FilePath, contents: Option<String>) {
        self.files.insert(path, contents);
    }
}
