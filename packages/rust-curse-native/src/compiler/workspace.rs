use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    string::FromUtf8Error,
};

use ra_ap_cfg::CfgOptions;
use ra_ap_edition::Edition as HirEdition;
use ra_ap_ide_db::{
    ChangeWithProcMacros,
    base_db::{
        AbsPathBuf, CrateDisplayName, CrateGraphBuilder as HirCrateGraphBuilder, CrateName,
        CrateOrigin, CrateWorkspaceData, DependencyBuilder, Env, FileSet, LangCrateOrigin,
        LibraryRoots, SourceDatabase, SourceRoot,
    },
};
use ra_ap_vfs::{Change, Vfs};
use thiserror::Error;
use triomphe::Arc;

use crate::compiler::{
    inputs::{CrateApiDiscovery, CrateApiImportMode, FfiLayout, Platform, WasmEnv::Web},
    path::{FilePath, FilePathError},
};

use super::{
    db::RootDatabase,
    inputs::{CrateMetadata, CratesMetadata},
};

/// Workspace exposing convinient methods to setup an environment for rust
/// analyzer database and expose it via [`Self::db`] and [`Self::db_mut`].
///
/// Useful for testint and quick project bootstraping. Alternatively, you can
/// use the rust-analyzer [`RootDatabase`] directly and settings rust-curse
/// specific inputs using [`super::inputs`].
pub struct Workspace {
    db: RootDatabase,
    vfs: Vfs,
}

impl Workspace {
    /// Creates a new empty default workspace.
    pub fn new() -> Self {
        let mut workspace = Self {
            db: RootDatabase::default(),
            vfs: Vfs::default(),
        };

        CratesMetadata::set(
            &mut workspace.db,
            CratesMetadata {
                crates: HashMap::new(),
            },
        );

        Platform::set(&mut workspace.db, Platform::Wasm { target: Web });
        FfiLayout::set(
            &mut workspace.db,
            FfiLayout {
                ffi_js_file: FilePath::new_virtual("/@ffi").unwrap(),
            },
        );

        workspace
    }

    /// Applies an external change to workspace.
    ///
    /// # Example:
    /// ```ignore
    /// use rust_curse::compiler::workspace::{
    ///     Workspace, CrateApiDiscovery, CrateApiImportMode, CrateGraphBuilder,
    ///     Edition, FilePath, WorkspaceChange, WorkspaceChangeError, WorkspaceCrate,
    /// };
    ///
    /// let mut workspace = Workspace::new();
    ///
    /// let root = FilePath::new_virtual("/src/lib.rs")?;
    /// let mut change = WorkspaceChange::new();
    /// change.set_local_file(root.clone(), Some("pub fn greet() {}".into()));
    ///
    /// let mut crates = CrateGraphBuilder::default();
    /// crates.add_crate(WorkspaceCrate {
    ///     name: "example".into(),
    ///     edition: Edition::E2024,
    ///     root_file: root,
    ///     version: "0.1.0".into(),
    ///     kind: WorkspaceCrateKind::Local,
    ///     api_discovery: CrateApiDiscovery::Eager,
    ///     api_import_mode: CrateApiImportMode::Relative,
    /// });
    /// change.set_crates(crates);
    /// workspace.apply_change(change)?;
    /// # Ok::<(), WorkspaceChangeError>(())
    /// ```
    pub fn apply_change(&mut self, change: WorkspaceChange) -> Result<(), WorkspaceChangeError> {
        // note: technically speaking, it's possible to just expose the database and
        // CratesMetadata
        change.apply(&mut self.db, &mut self.vfs)
    }

    /// Returns the database used for analysis and compilation queries.
    pub fn db(&self) -> &RootDatabase {
        &self.db
    }

    /// Changes a compilation target platform.
    pub fn change_platform(&mut self, platform: Platform) {
        Platform::set(&mut self.db, platform);
    }

    /// Sets a FFI layout to a custom one.
    pub fn change_ffi_layout(&mut self, ffi_layout: FfiLayout) {
        FfiLayout::set(&mut self.db, ffi_layout);
    }

    /// Returns the database mutably, for setting external compiler inputs.
    ///
    /// **Dangerous:** File and crate graph changes must still go through
    /// [`Workspace::apply_change`] so the database inputs and underlying VFS
    /// stay in sync. In case you are integrating rust-curse with other project
    /// using rust-analyzer hir database, you probably do not want to use
    /// [`Workspace`] at all and should just directly set singleton [`inputs`]
    /// such as [`CratesMetadata`](inputs::CratesMetadata).
    pub fn db_mut(&mut self) -> &mut RootDatabase {
        &mut self.db
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct WorkspaceChange {
    files: HashMap<FilePath, WorkspaceFileChange>,
    crate_graph: Option<CrateGraphBuilder>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct WorkspaceFileChange {
    contents: Option<String>,
    locality: FileLocality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileLocality {
    Local,
    Library,
}

impl WorkspaceChange {
    /// Crates new empty workspace change.
    pub fn new() -> Self {
        Default::default()
    }

    /// Sets an editable workspace file. Set content to `None` to remove it.
    pub fn set_local_file(&mut self, path: FilePath, contents: Option<String>) {
        self.set_file(path, contents, FileLocality::Local);
    }

    /// Sets an effectively immutable dependency file. Set content to `None` to
    /// remove it.
    pub fn set_library_file(&mut self, path: FilePath, contents: Option<String>) {
        self.set_file(path, contents, FileLocality::Library);
    }

    fn set_file(&mut self, path: FilePath, contents: Option<String>, locality: FileLocality) {
        self.files
            .entry(path)
            .and_modify(|change| {
                change.contents.clone_from(&contents);
                if locality == FileLocality::Local {
                    change.locality = FileLocality::Local;
                }
            })
            .or_insert(WorkspaceFileChange { contents, locality });
    }

    pub fn set_crates(&mut self, crate_graph: CrateGraphBuilder) {
        self.crate_graph = Some(crate_graph);
    }

    pub(super) fn apply(
        self,
        db: &mut RootDatabase,
        vfs: &mut Vfs,
    ) -> Result<(), WorkspaceChangeError> {
        let mut library_file_ids = HashSet::new();
        if let Some(roots) = LibraryRoots::try_get(db) {
            for root in roots.roots(db) {
                library_file_ids.extend(db.source_root(*root).source_root(db).iter());
            }
        }
        let mut changed_file_locality = HashMap::with_capacity(self.files.len());

        for (path, file_change) in self.files {
            changed_file_locality.insert(path.clone(), file_change.locality);
            vfs.set_file_contents(path.into(), file_change.contents.map(String::into_bytes));
        }

        let (crate_graph, crates_metadata) = match self.crate_graph {
            Some(crate_graph) => {
                let (crate_graph, crates_metadata) = Self::build_crate_graph(vfs, crate_graph)?;
                (Some(crate_graph), Some(crates_metadata))
            }
            None => (None, None),
        };

        let mut change = ChangeWithProcMacros::default();
        let mut local_files = FileSet::default();
        let mut library_files = FileSet::default();

        for (file_id, path) in vfs.iter() {
            let file_path = FilePath::from(path.clone());
            let locality = changed_file_locality
                .get(&file_path)
                .copied()
                .unwrap_or_else(|| {
                    if library_file_ids.contains(&file_id) {
                        FileLocality::Library
                    } else {
                        FileLocality::Local
                    }
                });
            match locality {
                FileLocality::Library => library_files.insert(file_id, path.clone()),
                FileLocality::Local => local_files.insert(file_id, path.clone()),
            }
        }
        change.set_roots(vec![
            SourceRoot::new_local(local_files),
            SourceRoot::new_library(library_files),
        ]);

        for changed_file in vfs.take_changes().into_values() {
            let contents = match changed_file.change {
                Change::Create(contents, _) | Change::Modify(contents, _) => {
                    Some(String::from_utf8(contents)?)
                }
                Change::Delete => None,
            };
            change.change_file(changed_file.file_id, contents);
        }

        if let Some(crate_graph) = crate_graph {
            change.set_crate_graph(crate_graph);
        }
        db.apply_change(change);

        if let Some(crates_metadata) = crates_metadata {
            CratesMetadata::set(db, crates_metadata);
        }
        Ok(())
    }

    fn build_crate_graph(
        vfs: &Vfs,
        crate_graph: CrateGraphBuilder,
    ) -> Result<(HirCrateGraphBuilder, CratesMetadata), WorkspaceChangeError> {
        let CrateGraphBuilder { crates, deps } = crate_graph;
        let mut hir_graph = HirCrateGraphBuilder::default();
        let mut hir_ids = Vec::with_capacity(crates.len());
        let mut metadata = HashMap::with_capacity(crates.len());
        // rust-analyzer requires this field even when proc macros are disabled.
        // Keep it synthetic so compiler setup never reads the host environment.
        let synthetic_root = if cfg!(windows) { r"C:\" } else { "/" };
        let proc_macro_cwd = Arc::new(AbsPathBuf::assert_utf8(PathBuf::from(synthetic_root)));
        let workspace_data = Arc::new(CrateWorkspaceData {
            target: Err("target data is not configured".into()),
            toolchain: None,
        });

        for crate_ in crates {
            let root_path = crate_.root_file.clone();
            let root_vfs_path = root_path.clone().into();
            let (root_file_id, _) = vfs.file_id(&root_vfs_path).ok_or_else(|| {
                WorkspaceChangeError::MissingCrateRoot {
                    path: root_path.clone(),
                }
            })?;
            let crate_name = CrateName::normalize_dashes(&crate_.name);
            let origin = match crate_.kind {
                WorkspaceCrateKind::Lang => CrateOrigin::Lang(match crate_.name.as_str() {
                    "alloc" => LangCrateOrigin::Alloc,
                    "core" => LangCrateOrigin::Core,
                    "proc_macro" => LangCrateOrigin::ProcMacro,
                    "std" => LangCrateOrigin::Std,
                    _ => panic!("unsupported sysroot crate"),
                }),
                WorkspaceCrateKind::Local => CrateOrigin::Local {
                    repo: None,
                    name: Some(crate_name.symbol().clone()),
                },
                WorkspaceCrateKind::Dependency => CrateOrigin::Library {
                    repo: None,
                    name: crate_name.symbol().clone(),
                },
            };
            let hir_id = hir_graph.add_crate_root(
                root_file_id,
                crate_.edition.into(),
                Some(CrateDisplayName::from(crate_name.clone())),
                Some(crate_.version),
                CfgOptions::default(),
                None,
                Env::default(),
                origin,
                Vec::new(),
                false,
                proc_macro_cwd.clone(),
                workspace_data.clone(),
            );

            metadata.insert(
                root_path,
                CrateMetadata {
                    api_discovery: crate_.api_discovery,
                    api_import_mode: crate_.api_import_mode,
                },
            );
            hir_ids.push(hir_id);
        }

        for dependency in deps {
            let from = *hir_ids.get(dependency.from_crate.0).ok_or(
                WorkspaceChangeError::InvalidCrateId {
                    role: "source",
                    id: dependency.from_crate,
                },
            )?;
            let to = *hir_ids.get(dependency.to_crate.0).ok_or(
                WorkspaceChangeError::InvalidCrateId {
                    role: "target",
                    id: dependency.to_crate,
                },
            )?;
            let name = CrateName::new(&dependency.name).map_err(|name| {
                WorkspaceChangeError::InvalidDependencyName {
                    name: name.to_owned(),
                }
            })?;

            hir_graph
                .add_dep(from, DependencyBuilder::new(name, to))
                .map_err(|error| WorkspaceChangeError::CyclicDependency {
                    message: error.to_string(),
                })?;
        }

        Ok((hir_graph, CratesMetadata { crates: metadata }))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrateGraphBuilder {
    crates: Vec<WorkspaceCrate>,
    deps: Vec<WorkspaceCrateDependency>,
}

impl CrateGraphBuilder {
    pub fn add_crate(&mut self, crate_: WorkspaceCrate) -> CrateBuilderId {
        let id = CrateBuilderId(self.crates.len());
        self.crates.push(crate_);
        id
    }

    pub fn add_dependency(&mut self, dependency: WorkspaceCrateDependency) {
        self.deps.push(dependency)
    }
}

/// Temporary crate id used to crate a dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct CrateBuilderId(usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceCrate {
    pub name: String,
    pub edition: Edition,
    pub root_file: FilePath,
    pub version: String,
    pub kind: WorkspaceCrateKind,
    pub api_discovery: CrateApiDiscovery,
    pub api_import_mode: CrateApiImportMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::Update)]
pub enum WorkspaceCrateKind {
    Local,
    Lang,
    Dependency,
}

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct WorkspaceCrateDependency {
    pub name: String,
    pub from_crate: CrateBuilderId,
    pub to_crate: CrateBuilderId,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[non_exhaustive]
pub enum Edition {
    E2015,
    E2018,
    E2021,
    #[default]
    E2024,
}

impl From<Edition> for HirEdition {
    fn from(value: Edition) -> Self {
        match value {
            Edition::E2015 => Self::Edition2015,
            Edition::E2018 => Self::Edition2018,
            Edition::E2021 => Self::Edition2021,
            Edition::E2024 => Self::Edition2024,
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WorkspaceChangeError {
    #[error("crate root file is not present in the compiler workspace: {path:?}")]
    MissingCrateRoot { path: FilePath },

    #[error("dependency {role} crate id is out of bounds: {id:?}")]
    InvalidCrateId {
        role: &'static str,
        id: CrateBuilderId,
    },

    #[error("dependency name is not a valid Rust crate name: {name}")]
    InvalidDependencyName { name: String },

    #[error("crate dependency introduces a cycle: {message}")]
    CyclicDependency { message: String },

    #[error("compiler workspace contents are not valid UTF-8")]
    InvalidFileContents(#[from] FromUtf8Error),

    #[error(transparent)]
    InvalidFilePath(#[from] FilePathError),
}
