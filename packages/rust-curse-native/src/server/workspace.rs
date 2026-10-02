use std::path::{Path, PathBuf};

use miette::Diagnostic;
use ra_ap_ide_db::{ChangeWithProcMacros, RootDatabase};
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace};
use ra_ap_project_model::{
    CargoConfig, ProjectManifest, ProjectWorkspace, ProjectWorkspaceKind, RustLibSource,
};
use ra_ap_vfs::{AbsPathBuf, Change, Vfs, VfsPath};
use thiserror::Error;

use crate::{
    compiler::{inputs::LocalCrates, path::FilePath},
    server::{
        ServerChange,
        project::{Project, ProjectConfig},
    },
};

#[derive(Debug)]
pub struct ServerWorkspace {
    db: RootDatabase,
    vfs: Vfs,
    root_file: PathBuf,
    manifest_path: PathBuf,
    package_name: String,
}

impl ServerWorkspace {
    /// Loads a Cargo manifest and marks its package targets as local.
    /// The manifest's containing directory is the workspace root.
    pub fn load(manifest_path: PathBuf) -> Result<Self, WorkspaceError> {
        let _span = tracing::info_span!("Rust workspace loading").entered();

        let cargo_config = CargoConfig {
            sysroot: Some(RustLibSource::Discover),
            ..Default::default()
        };

        let load_config = LoadCargoConfig {
            load_out_dirs_from_check: false,
            with_proc_macro_server: ProcMacroServerChoice::None,
            prefill_caches: false,
            num_worker_threads: 1,
            proc_macro_processes: 1,
        };

        let manifest =
            ProjectManifest::from_manifest_file(AbsPathBuf::assert_utf8(manifest_path.clone()))
                .map_err(|source| WorkspaceError::Manifest {
                    path: manifest_path.clone(),
                    source,
                })?;

        let hir_ws = ProjectWorkspace::load(manifest, &cargo_config, &|progress| {
            tracing::info!("Workspace progress: {progress}");
        })
        .map_err(|source| WorkspaceError::Project {
            path: manifest_path.clone(),
            source,
        })?;

        let cargo = match &hir_ws.kind {
            ProjectWorkspaceKind::Cargo { cargo, .. } => cargo,
            ProjectWorkspaceKind::DetachedFile {
                cargo: Some((cargo, ..)),
                ..
            } => cargo,

            _ => {
                return Err(WorkspaceError::UnsupportedManifest {
                    path: manifest_path,
                });
            }
        };

        let package = cargo
            .packages()
            .find(|&package| {
                let path: &Path = cargo[package].manifest.as_ref();
                path == manifest_path
            })
            .ok_or_else(|| WorkspaceError::MissingPackage {
                path: manifest_path.clone(),
            })?;

        let root = cargo[package]
            .targets
            .iter()
            .map(|&target| FilePath::from(VfsPath::from(cargo[target].root.clone())))
            .next()
            .ok_or_else(|| WorkspaceError::MissingTarget {
                path: manifest_path.clone(),
            })?;

        let root_file = root.as_path().to_owned();
        let package_name = cargo[package].name.clone();

        let (mut db, mut vfs, _) = load_workspace(hir_ws, &cargo_config.extra_env, &load_config)
            .map_err(|source| WorkspaceError::Database {
                path: manifest_path.clone(),
                source,
            })?;

        LocalCrates::set(&mut db, [root]);
        // Initial loading already applied these changes to the database.
        vfs.take_changes();

        Ok(Self {
            manifest_path,
            db,
            vfs,
            root_file,
            package_name,
        })
    }

    /// Applies Rust source changes and reloads on Cargo manifest or lockfile
    /// changes. All other files are ignored. Returns `true` if the workspace
    /// was changed, `false` otherwise.
    pub fn change(&mut self, change: WorkspaceChange) -> Result<bool, WorkspaceError> {
        let path = Path::new(&change.file_path);
        if matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some("Cargo.toml" | "Cargo.lock")
        ) {
            let workspace = Self::load(self.manifest_path.clone()).map_err(|source| {
                WorkspaceError::Reload {
                    path: path.to_owned(),
                    source: Box::new(source),
                }
            })?;
            *self = workspace;

            return Ok(true);
        }

        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            return Ok(false);
        }

        self.vfs.set_file_contents(
            VfsPath::new_real_path(change.file_path),
            change.contents.map(String::into_bytes),
        );

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

        Ok(changed)
    }

    pub fn project<'db>(&self, db: &'db RootDatabase) -> Project<'db> {
        assert!(
            self.manifest_path.is_file() && self.manifest_path.file_name().unwrap() == "Cargo.toml"
        );
        assert!(self.root_file.is_file() && self.root_file.file_name().unwrap() == "lib.rs");

        Project::new(
            db,
            ProjectConfig {
                manifest_path: self.manifest_path.clone(),
                root_file: self.root_file.clone(),
                package_name: self.package_name.clone(),
            },
        )
    }

    /// Runs a complete compiler query with rust-analyzer's database attached.
    pub fn with_db<R>(&self, f: impl FnOnce(&RootDatabase) -> R) -> R {
        ra_ap_hir::attach_db(&self.db, || f(&self.db))
    }

    /// Updates database inputs without retaining a shared database attachment.
    pub fn with_db_mut<R>(&mut self, f: impl FnOnce(&mut RootDatabase) -> R) -> R {
        f(&mut self.db)
    }
}

pub struct WorkspaceChange {
    pub file_path: String,
    pub contents: Option<String>,
}

impl From<ServerChange> for WorkspaceChange {
    fn from(value: ServerChange) -> Self {
        WorkspaceChange {
            file_path: value.file_path,
            contents: value.contents,
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
pub enum WorkspaceError {
    #[error("failed to read project manifest {}", path.display())]
    Manifest {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },
    #[error("failed to load project workspace for {}", path.display())]
    Project {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },
    #[error("rust-curse doesn't support rust-project.json ({})", path.display())]
    UnsupportedManifest { path: PathBuf },
    #[error("no Cargo package matches manifest {}", path.display())]
    MissingPackage { path: PathBuf },
    #[error("no target found for package in {}", path.display())]
    MissingTarget { path: PathBuf },
    #[error("failed to load workspace database for {}", path.display())]
    Database {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },
    #[error("failed to reload workspace after changing {}", path.display())]
    Reload {
        path: PathBuf,
        #[source]
        source: Box<WorkspaceError>,
    },
}
