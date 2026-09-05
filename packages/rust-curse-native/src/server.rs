use napi_derive::napi;

mod paths;

// #[napi(object)]
// #[derive(Debug)]
// pub struct ServerCfg {
//     /// A root directory of the JS project.
//     pub root_dir: String,
//     /// Manifest path of the "main" crate relative to the [`Self::root_dir`].
//     pub manifest_path: String,
//     /// Path to the output dirrectory relative to the [`Self::root_dir`].
//     pub out_dir_rel: String,
// }

// #[derive(Debug, Default)]
// pub struct Server {
//     workspace: Workspace,
//     // reconciler UpdateReconciler,
// }

// impl Server {
//     pub fn configure(&mut self, cfg: ServerCfg) {
//         // WorkspacePaths::new(WorkspacePathsOpts {
//         //     root_dir: PathBuf::from(&cfg.root_dir),
//         //     manifest_path_rel: PathBuf::from(&cfg.manifest_path),
//         //     out_dir_rel: PathBuf::from(&cfg.out_dir_rel),
//         // })
//         // .set(self.workspace.db_mut());

//         // self.reconciler.rescan(&mut db, &mut workspace);
//         //
//         //
//         // ShimCrate::dump(&workspace.db);
//         // CursedCrate::rebuild(&workspace.db);
//         //
//         // TsTree::dump(&workspace.db);

//         // let shim_crate = Rust::get(workspace.crate).shim();
//         // ShimCrate::render_lib();
//         // ShimCrate::render_build() -> Option;
//         // ShimCrate::render_manifest();
//     }

//     pub fn update(&mut self) {
//         todo!()
//     }

//     pub fn build(&mut self) {
//         todo!()
//     }

//     pub fn load(&mut self) {
//         todo!()
//     }
// }
