mod check;
mod codegen;
mod db;
mod diag;
mod inputs;
mod ir;
mod mangle;
mod path;

#[cfg(test)]
mod test;

use std::{collections::HashMap, path::PathBuf, string::FromUtf8Error};

use ra_ap_cfg::CfgOptions;
use ra_ap_edition::Edition as HirEdition;
use ra_ap_ide_db::{
    ChangeWithProcMacros,
    base_db::{
        AbsPathBuf, CrateDisplayName, CrateGraphBuilder as HirCrateGraphBuilder, CrateName,
        CrateOrigin, CrateWorkspaceData, DependencyBuilder, Env, FileSet, SourceRoot,
    },
};
use ra_ap_vfs::{Change, Vfs};
use thiserror::Error;
use triomphe::Arc;

use self::{
    db::RootDatabase,
    diag::Diag,
    inputs::{CrateMetadata, CratesMetadata},
    ir::{rust::Rust, shim::ShimMod},
};

pub use self::{
    diag::Theme,
    inputs::{CrateApiDiscovery, CrateApiImportMode, FfiLayout, Platform, WasmEnv},
    path::{FilePath, FilePathError},
};

/// An incremental rust-curse compiler.
pub struct Compiler {
    db: RootDatabase,
    vfs: Vfs,
}

impl Compiler {
    pub fn new(opts: CompilerOpts) -> Self {
        let mut compiler = Self {
            db: RootDatabase::default(),
            vfs: Vfs::default(),
        };

        // it's mandatory to have some options set in the database,
        // otherwise compilation will panic on run.
        compiler.set_opts(opts);
        CratesMetadata::set(
            &mut compiler.db,
            CratesMetadata {
                crates: HashMap::new(),
            },
        );

        compiler
    }

    /// Gets currently applied options.
    pub fn opts(&self) -> CompilerOpts {
        CompilerOpts {
            platform: Platform::get(&self.db),
            ffi_layout: FfiLayout::get(&self.db),
        }
    }

    /// Sets new options.
    pub fn set_opts(&mut self, opts: CompilerOpts) {
        Platform::set(&mut self.db, opts.platform);
        FfiLayout::set(&mut self.db, opts.ffi_layout);
    }

    /// Changes the workspace (source files, crates) available to the compiler.
    ///
    /// # Example:
    /// ```ignore
    /// use rust_curse::compiler::{
    ///     Compiler, CompilerOpts, CrateApiDiscovery, CrateApiImportMode,
    ///     CrateGraphBuilder, Edition, FfiLayout, FilePath, Platform,
    ///     WorkspaceChangeError, WorkspaceCrate,
    /// };
    ///
    /// let ffi_js_file = FilePath::new_virtual("/generated/ffi.js")?;
    /// let mut compiler = Compiler::new(CompilerOpts {
    ///     platform: Platform::Node,
    ///     ffi_layout: FfiLayout { ffi_js_file },
    /// });
    ///
    /// compiler.change_workspace(|change| {
    ///     let root = FilePath::new_virtual("/src/lib.rs")?;
    ///     change.set_file(root.clone(), Some("pub fn greet() {}".into()));
    ///
    ///     let mut crates = CrateGraphBuilder::default();
    ///     crates.add_crate(WorkspaceCrate {
    ///         name: "example".into(),
    ///         edition: Edition::E2024,
    ///         root_file: root,
    ///         version: "0.1.0".into(),
    ///         api_discovery: CrateApiDiscovery::Eager,
    ///         api_import_mode: CrateApiImportMode::Relative,
    ///     });
    ///     change.set_crates(crates);
    ///     Ok(())
    /// })?;
    /// # Ok::<(), WorkspaceChangeError>(())
    /// ```
    pub fn change_workspace(
        &mut self,
        once: impl FnOnce(&mut WorkspaceChange) -> Result<(), WorkspaceChangeError>,
    ) -> Result<(), WorkspaceChangeError> {
        let mut change = WorkspaceChange {
            files: HashMap::new(),
            crate_graph: None,
        };
        once(&mut change)?;
        change.apply(&mut self.db, &mut self.vfs)
    }

    /// Compiles a shim code and collects diagnostics.
    pub fn compile_shim(&self) -> ShimCompilationOutput {
        let rust = Rust::get(&self.db);
        let diagnostics = rust
            .crates(&self.db)
            .iter()
            .flat_map(|crate_| crate_.check(&self.db))
            .cloned()
            .map(CompilerDiagnostic::from)
            .collect();
        let shim = rust.shim(&self.db).render(&self.db).to_owned();

        ShimCompilationOutput { diagnostics, shim }
    }

    /// Compiles a js code for a given rust module file.
    ///
    /// Compilation of js code cannot emit new diagnostics besides those
    /// returned by [`Self::compile_shim`] function. Returns [`None`] when the
    /// path does not identify a Rust module in the compiler workspace.
    pub fn compile_js(&self, path: FilePath) -> Option<String> {
        self.shim_mod(path)
            .map(|mod_| mod_.facade(&self.db).render(&self.db).to_owned())
    }

    /// Compiles a ts code for a given rust module file.
    ///
    /// Compilation of ts code cannot emit new diagnostics besides those
    /// returned by [`Self::compile_shim`] function. Returns [`None`] when the
    /// path does not identify a Rust module in the compiler workspace.
    pub fn compile_ts(&self, path: FilePath) -> Option<String> {
        self.shim_mod(path)
            .map(|mod_| mod_.decals(&self.db).render_ts(&self.db).to_owned())
    }

    /// Compiles a ts sourcemap file for a given rust module file.
    ///
    /// Compilation of ts sourcemap file cannot emit new diagnostics besides
    /// those returned by [`Self::compile_shim`] function. Returns [`None`] when
    /// the path does not identify a Rust module in the compiler workspace.
    pub fn compile_ts_sourcemap(&self, path: FilePath) -> Option<String> {
        self.shim_mod(path)
            .map(|mod_| mod_.decals(&self.db).render_sourcemap(&self.db).to_owned())
    }

    fn shim_mod(&self, path: FilePath) -> Option<ShimMod<'_>> {
        Rust::get(&self.db)
            .shim(&self.db)
            .crates(&self.db)
            .iter()
            .flat_map(|crate_| crate_.mods(&self.db))
            .find(|mod_| mod_.facade(&self.db).file_path(&self.db) == &path)
            .copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerOpts {
    /// Target platform used for FFI.
    pub platform: Platform,
    /// Layout of generated ffi files.
    pub ffi_layout: FfiLayout,
}

pub struct WorkspaceChange {
    files: HashMap<FilePath, Option<String>>,
    crate_graph: Option<CrateGraphBuilder>,
}

impl WorkspaceChange {
    /// Sets a file content. Set content to `None` to remove a file.
    pub fn set_file(&mut self, path: FilePath, contents: Option<String>) {
        self.files.insert(path, contents);
    }

    pub fn set_crates(&mut self, crate_graph: CrateGraphBuilder) {
        self.crate_graph = Some(crate_graph);
    }

    fn apply(self, db: &mut RootDatabase, vfs: &mut Vfs) -> Result<(), WorkspaceChangeError> {
        for (path, contents) in self.files {
            vfs.set_file_contents(path.into(), contents.map(String::into_bytes));
        }

        let (crate_graph, crates_metadata) = match self.crate_graph {
            Some(crate_graph) => {
                let (crate_graph, crates_metadata) = Self::build_crate_graph(vfs, crate_graph)?;
                (Some(crate_graph), Some(crates_metadata))
            }
            None => (None, None),
        };

        let mut change = ChangeWithProcMacros::default();
        let mut files = FileSet::default();
        for (file_id, path) in vfs.iter() {
            files.insert(file_id, path.clone());
        }
        change.set_roots(vec![SourceRoot::new_local(files)]);

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
            let hir_id = hir_graph.add_crate_root(
                root_file_id,
                crate_.edition.into(),
                Some(CrateDisplayName::from(crate_name.clone())),
                Some(crate_.version),
                CfgOptions::default(),
                None,
                Env::default(),
                CrateOrigin::Local {
                    repo: None,
                    name: Some(crate_name.symbol().clone()),
                },
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
    pub api_discovery: CrateApiDiscovery,
    pub api_import_mode: CrateApiImportMode,
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

#[derive(Debug, Clone)]
pub struct ShimCompilationOutput {
    /// Diagnostics emitted during compilation.
    ///
    /// A field contains all diagnostics emitted by current workspace regardless
    /// of whether they were caused by the latest changes or not.
    pub diagnostics: Vec<CompilerDiagnostic>,
    /// Compiled shim code.
    pub shim: String,
}

#[derive(Clone)]
pub struct CompilerDiagnostic {
    inner: Diag,
}

impl CompilerDiagnostic {
    pub fn render(&self, theme: Theme) -> Result<String, std::fmt::Error> {
        self.inner.render(theme)
    }
}

impl From<Diag> for CompilerDiagnostic {
    fn from(inner: Diag) -> Self {
        Self { inner }
    }
}

impl std::fmt::Debug for CompilerDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompilerDiagnostic")
            .field("message", &self.inner.inner.to_string())
            .finish()
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
