//! Cargo and filesystem boundary for the compiler's immutable analysis snapshot.

use std::{path::Path, sync::Arc};

use compiler::db::RootDatabase;
use ra_ap_base_db::{
    Crate, CrateBuilderId, CrateGraphBuilder, FileChange, SourceDatabase, SourceRoot,
};
use ra_ap_hir_expand::proc_macro::CrateProcMacros;
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, ProjectFolders, load_workspace};
use ra_ap_project_model::{CargoConfig, ProjectManifest, ProjectWorkspace, RustLibSource};
use ra_ap_vfs::{AbsPathBuf, FileId, VfsPath};
use salsa::Setter;

use super::write_if_changed;

pub(super) struct Snapshot {
    crate_graph: CrateGraphBuilder,
    source_roots: Vec<SourceRoot>,
    sources: Vec<(FileId, Arc<str>)>,
    proc_macros: Vec<(CrateBuilderId, triomphe::Arc<CrateProcMacros>)>,
    expand_proc_attr_macros: bool,
    template: CrateBuilderId,
}

impl Snapshot {
    pub(super) fn apply(&self, db: &mut RootDatabase) -> Crate {
        let mut change = FileChange::default();
        change.set_roots(self.source_roots.clone());
        change.set_crate_graph(self.crate_graph.clone());

        for (file, text) in &self.sources {
            change.change_file(*file, Some(text.to_string()));
        }

        let crates = change.apply(db).expect("snapshot contains a crate graph");

        ra_ap_hir::ProcMacros::get(db).set_by_crate(db).to(self
            .proc_macros
            .iter()
            .map(|(id, macros)| (crates[id], macros.clone()))
            .collect());
        ra_ap_hir::db::set_expand_proc_attr_macros(db, self.expand_proc_attr_macros);

        crates[&self.template]
    }
}

pub(super) fn load(
    directory: &Path,
    manifest: &str,
    target: Option<String>,
    proc_macros: bool,
) -> Result<Snapshot, String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    write_if_changed(&directory.join("Cargo.toml"), manifest.as_bytes())
        .and_then(|_| write_if_changed(&directory.join("lib.rs"), b""))
        .map_err(|error| error.to_string())?;
    let config = CargoConfig {
        sysroot: Some(RustLibSource::Discover),
        target,
        ..Default::default()
    };
    let manifest =
        ProjectManifest::from_manifest_file(AbsPathBuf::assert_utf8(directory.join("Cargo.toml")))
            .map_err(|error| error.to_string())?;
    let mut workspace =
        ProjectWorkspace::load(manifest, &config, &|_| {}).map_err(|error| error.to_string())?;
    if proc_macros {
        let build_scripts = workspace
            .run_build_scripts(&config, &|_| {})
            .map_err(|error| error.to_string())?;
        if let Some(error) = build_scripts.error() {
            return Err(error.to_owned());
        }
        workspace.set_build_scripts(build_scripts);
    }

    let folders = ProjectFolders::new(std::slice::from_ref(&workspace), &[], None);
    let (source_db, vfs, _) = load_workspace(
        workspace.clone(),
        &config.extra_env,
        &LoadCargoConfig {
            load_out_dirs_from_check: proc_macros,
            with_proc_macro_server: if proc_macros {
                ProcMacroServerChoice::Sysroot
            } else {
                ProcMacroServerChoice::None
            },
            prefill_caches: false,
            num_worker_threads: 1,
            proc_macro_processes: 1,
        },
    )
    .map_err(|error| error.to_string())?;
    let root_file = vfs
        .file_id(&VfsPath::new_real_path(
            directory.join("lib.rs").to_string_lossy().into_owned(),
        ))
        .ok_or("analysis crate root was not loaded")?
        .0;
    let (crate_graph, _) = workspace.to_crate_graph(
        &mut |path| {
            vfs.file_id(&VfsPath::from(path.to_path_buf()))
                .map(|(id, _)| id)
        },
        &config.extra_env,
    );
    let template = crate_graph
        .iter()
        .find(|id| crate_graph[*id].basic.root_file_id == root_file)
        .ok_or("analysis template crate was not loaded")?;

    let loaded_macros = ra_ap_hir::ProcMacros::get(&source_db).by_crate(&source_db);
    let expanders = loaded_macros
        .iter()
        .map(|(crate_, macros)| {
            let root = crate_.data(&source_db).root_file_id;
            let id = crate_graph
                .iter()
                .find(|id| crate_graph[*id].basic.root_file_id == root)
                .expect("loaded proc-macro crate is in the rebuilt graph");
            (id, macros.clone())
        })
        .collect();

    let sources = vfs
        .iter()
        .map(|(file, _)| {
            let text = source_db.file_text(file);
            (file, Arc::<str>::from(text.text(&source_db).as_ref()))
        })
        .collect();
    Ok(Snapshot {
        crate_graph,
        source_roots: folders.source_root_config.partition(&vfs),
        sources,
        proc_macros: expanders,
        expand_proc_attr_macros: proc_macros,
        template,
    })
}
