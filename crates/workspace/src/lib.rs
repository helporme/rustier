//! Application-facing access to the Rustier compiler database.

use std::{collections::BTreeMap, fs, io, path::PathBuf};

use ra_ap_base_db::{FileChange, SourceDatabase};
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, ProjectFolders, load_workspace};
use ra_ap_project_model::{
    CargoConfig, CargoFeatures, ProjectManifest, ProjectWorkspace, RustLibSource,
};
use ra_ap_vfs::{AbsPathBuf, VfsPath};
use rustier_compiler::{
    codegen::manifest::{Dependency, render_manifest},
    db::RootDatabase,
    ir::runtime::RuntimeAnalysis,
};
use salsa::Setter;

pub mod build;

/// Options for the generated analysis crate.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct WorkspaceOpts {
    /// Path to the directory for the generated workspace files.
    pub tmp_path: PathBuf,
    /// Dependencies from local paths or registry versions.
    pub dependency_crates: BTreeMap<String, Dependency>,
    /// Cargo features to enable, such as `my_crate/feature`. Defaults remain enabled.
    pub features: Vec<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

impl WorkspaceOpts {
    /// Decodes workspace options from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Failure to create or load the generated analysis crate.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("cannot load analysis workspace: {0}")]
    Load(#[from] anyhow::Error),
}

/// Owns a compiler database loaded from a generated Cargo crate.
pub struct Workspace {
    db: RootDatabase,
}

impl Workspace {
    /// Generate and loads a workspace crate.
    pub fn load(opts: WorkspaceOpts) -> Result<Self, LoadError> {
        if !opts.tmp_path.is_absolute() || opts.tmp_path.to_str().is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "tmp_path must be an absolute UTF-8 path",
            )
            .into());
        }

        let mut crates = BTreeMap::new();
        for (name, mut dependency) in opts.dependency_crates {
            if let Dependency::Path { path, .. } = &mut dependency {
                *path = fs::canonicalize(&*path)?
                    .into_os_string()
                    .into_string()
                    .map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidInput, "crate paths must be UTF-8")
                    })?;
            }
            crates.insert(name, dependency);
        }

        let manifest = render_manifest(&crates);
        fs::create_dir_all(&opts.tmp_path)?;
        let directory = AbsPathBuf::assert_utf8(fs::canonicalize(&opts.tmp_path)?);
        fs::write(directory.join("Cargo.toml"), manifest)?;
        fs::write(directory.join("lib.rs"), "")?;

        let config = CargoConfig {
            sysroot: Some(RustLibSource::Discover),
            target: opts.target,
            extra_env: opts
                .env
                .into_iter()
                .map(|(name, value)| (name, Some(value)))
                .collect(),
            features: CargoFeatures::Selected {
                features: opts.features,
                no_default_features: false,
            },
            ..Default::default()
        };
        let manifest = ProjectManifest::from_manifest_file(directory.join("Cargo.toml"))?;
        let mut project = ProjectWorkspace::load(manifest, &config, &|_| {})?;
        let build_scripts = project.run_build_scripts(&config, &|_| {})?;
        if let Some(error) = build_scripts.error() {
            return Err(LoadError::Load(anyhow::anyhow!("{error}")));
        }
        project.set_build_scripts(build_scripts);

        let folders = ProjectFolders::new(std::slice::from_ref(&project), &[], None);
        let (source_db, vfs, _) = load_workspace(
            project.clone(),
            &config.extra_env,
            &LoadCargoConfig {
                load_out_dirs_from_check: true,
                with_proc_macro_server: ProcMacroServerChoice::Sysroot,
                prefill_caches: false,
                num_worker_threads: 1,
                proc_macro_processes: 1,
            },
        )?;

        let root_file = vfs
            .file_id(&VfsPath::from(directory.join("lib.rs")))
            .expect("generated analysis crate root is loaded")
            .0;

        let (graph, _) = project.to_crate_graph(
            &mut |path| {
                vfs.file_id(&VfsPath::from(path.to_path_buf()))
                    .map(|(id, _)| id)
            },
            &config.extra_env,
        );

        let template = graph
            .iter()
            .find(|id| graph[*id].basic.root_file_id == root_file)
            .expect("generated analysis crate is in the crate graph");

        let expanders: Vec<_> = ra_ap_hir::ProcMacros::get(&source_db)
            .by_crate(&source_db)
            .iter()
            .map(|(crate_, macros)| {
                let root = crate_.data(&source_db).root_file_id;
                let id = graph
                    .iter()
                    .find(|id| graph[*id].basic.root_file_id == root)
                    .expect("loaded proc-macro crate is in the rebuilt graph");
                (id, macros.clone())
            })
            .collect();

        let mut changes = FileChange::default();
        changes.set_roots(folders.source_root_config.partition(&vfs));
        changes.set_crate_graph(graph);

        for (file, _) in vfs.iter() {
            changes.change_file(
                file,
                Some(source_db.file_text(file).text(&source_db).to_string()),
            );
        }

        let mut db = RootDatabase::default();
        let crates = changes.apply(&mut db).expect("a crate graph was supplied");

        ra_ap_hir::ProcMacros::get(&db)
            .set_by_crate(&mut db)
            .to(expanders
                .into_iter()
                .map(|(id, macros)| (crates[&id], macros))
                .collect());
        ra_ap_hir::db::set_expand_proc_attr_macros(&mut db, true);
        RuntimeAnalysis::set(&mut db, crates[&template]);

        Ok(Self { db })
    }

    /// Runs a closure with the database attached to rust-analyzer's thread-local context.
    pub fn with_db<R>(&self, f: impl FnOnce(&RootDatabase) -> R) -> R {
        ra_ap_hir::attach_db(&self.db, || f(&self.db))
    }

    /// Provides mutable access for setting database inputs.
    ///
    /// Run queries requiring an attached database through [`Self::with_db`].
    pub fn with_db_mut<R>(&mut self, f: impl FnOnce(&mut RootDatabase) -> R) -> R {
        f(&mut self.db)
    }
}
