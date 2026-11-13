//! Cargo and filesystem boundary for the compiler's immutable analysis snapshot.

use std::{path::Path, sync::Arc};

use compiler::db::RootDatabase;
use ra_ap_base_db::{
    Crate, CrateBuilderId, CrateGraphBuilder, FileChange, SourceDatabase, SourceRoot,
};
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, ProjectFolders, load_workspace};
use ra_ap_project_model::{CargoConfig, ProjectManifest, ProjectWorkspace, RustLibSource};
use ra_ap_vfs::{AbsPathBuf, FileId, VfsPath};

use super::write_if_changed;

pub(super) struct Snapshot {
    crate_graph: CrateGraphBuilder,
    source_roots: Vec<SourceRoot>,
    sources: Vec<(FileId, Arc<str>)>,
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

        crates[&self.template]
    }
}

pub(super) fn load(
    directory: &Path,
    manifest: &str,
    target: Option<String>,
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
    let workspace =
        ProjectWorkspace::load(manifest, &config, &|_| {}).map_err(|error| error.to_string())?;
    let folders = ProjectFolders::new(std::slice::from_ref(&workspace), &[], None);
    // Do not run build scripts, cargo check, or a proc-macro server. In
    // particular, analysis never consumes output from the bridge cargo build.
    let (source_db, vfs, _) = load_workspace(
        workspace.clone(),
        &config.extra_env,
        &LoadCargoConfig {
            load_out_dirs_from_check: false,
            with_proc_macro_server: ProcMacroServerChoice::None,
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
        template,
    })
}
