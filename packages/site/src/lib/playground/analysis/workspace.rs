use compiler::{
    codegen::{TransportBackend, TransportBackendWasm, WasmImport},
    db::RootDatabase,
    diag::Theme,
    ir::{
        js::{JsFile, JsProgram},
        rs::RsProgram,
        runtime::Runtime,
    },
};
use ra_ap_base_db::{
    CrateDisplayName, CrateGraphBuilder, CrateName, CrateOrigin, CrateWorkspaceData,
    DependencyBuilder, Env, FileChange, SourceRoot,
    target::{Arch, TargetData},
};
use ra_ap_cfg::CfgOptions;
use ra_ap_hir::Symbol;
use ra_ap_syntax::Edition;
use ra_ap_vfs::{AbsPathBuf, FileId, VfsPath, file_set::FileSet};
use salsa::Setter;
use triomphe::Arc;

include!(concat!(env!("OUT_DIR"), "/sources.rs"));

pub struct Workspace {
    db: RootDatabase,
    program: JsProgram,
    rs_program: RsProgram,
}

pub struct Analysis {
    pub transformed_js: String,
    pub rust_glue: String,
    pub js_glue: String,
    pub diagnostics: String,
}

impl Workspace {
    pub fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        let cwd = Arc::new(unsafe {
            // wasm32 cannot validate absolute paths; RA only uses this virtual cwd.
            std::mem::transmute::<ra_ap_paths::Utf8PathBuf, AbsPathBuf>(
                ra_ap_paths::Utf8PathBuf::from("/playground"),
            )
        });
        #[cfg(not(target_arch = "wasm32"))]
        let cwd = Arc::new(AbsPathBuf::assert_utf8("/playground".into()));
        let workspace = Arc::new(CrateWorkspaceData {
            target: Ok(TargetData {
                data_layout: "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
                    .into(),
                arch: Arch::Wasm32,
            }),
            toolchain: Some(env!("RUSTIER_RUST_VERSION").parse().unwrap()),
        });
        let mut cfg = CfgOptions::default();
        for (key, value) in [
            ("target_arch", "wasm32"),
            ("target_os", "unknown"),
            ("target_family", "wasm"),
            ("target_pointer_width", "32"),
            ("target_endian", "little"),
            ("target_vendor", "unknown"),
            ("target_env", ""),
        ] {
            cfg.insert_key_value(Symbol::intern(key), Symbol::intern(value));
        }
        cfg.insert_atom(Symbol::intern("rust_analyzer"));

        let mut db = RootDatabase::default();
        let mut change = FileChange::default();
        let mut files = FileSet::default();
        let mut graph = CrateGraphBuilder::default();
        let mut roots = Vec::new();

        for (index, (path, text)) in std::iter::once(("/playground/lib.rs", ""))
            .chain(SOURCES.iter().copied())
            .enumerate()
        {
            let file = FileId::from_raw(index as u32);
            files.insert(file, VfsPath::new_virtual_path(path.into()));
            change.change_file(file, Some(text.into()));
            if let Some(name) = path
                .strip_suffix("/src/lib.rs")
                .map(|name| &name[1..])
                .or_else(|| (index == 0).then_some("playground"))
            {
                let origin = match name {
                    "core" | "alloc" | "std" => CrateOrigin::Lang(name.into()),
                    _ => CrateOrigin::Local {
                        repo: None,
                        name: None,
                    },
                };
                let crate_ = graph.add_crate_root(
                    file,
                    Edition::Edition2024,
                    Some(CrateDisplayName::from_canonical_name(name)),
                    None,
                    cfg.clone(),
                    None,
                    Env::default(),
                    origin,
                    Vec::new(),
                    name == "rustier_js_proc",
                    cwd.clone(),
                    workspace.clone(),
                );
                roots.push((name, crate_));
            }
        }
        for &(name, crate_) in &roots {
            for &(dependency, target) in &roots {
                if matches!(
                    (name, dependency),
                    ("alloc", "core")
                        | ("std", "core" | "alloc")
                        | ("rustier_runtime", "core" | "alloc" | "std")
                        | (
                            "rustier",
                            "core" | "std" | "rustier_runtime" | "rustier_js_proc"
                        )
                        | (
                            "playground",
                            "core"
                                | "alloc"
                                | "std"
                                | "rustier"
                                | "rustier_runtime"
                                | "rustier_js_proc"
                        )
                ) {
                    graph
                        .add_dep(
                            crate_,
                            DependencyBuilder::new(CrateName::new(dependency).unwrap(), target),
                        )
                        .unwrap();
                }
            }
        }
        let template = roots[0].1;
        change.set_roots(vec![SourceRoot::new_library(files)]);
        change.set_crate_graph(graph);
        let crates = change.apply(&mut db).unwrap();
        let rs_program = RsProgram::new(&db, crates[&template]);
        let program = JsProgram::new(&db, Vec::new());

        Self {
            db,
            program,
            rs_program,
        }
    }

    pub fn analyze(&mut self, code: String) -> Analysis {
        let file = JsFile::new(&self.db, "/playground/index.ts", code);
        self.program.set_files(&mut self.db).to(vec![file]);
        let db = &self.db;

        ra_ap_hir::attach_db(db, || {
            let mut diagnostics = file
                .diagnostics(db)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let runtime = Runtime::new(db, self.program, self.rs_program);
            diagnostics.extend(runtime.check(db));
            let backend = TransportBackend::Wasm(TransportBackendWasm {
                mem64: false,
                import: WasmImport::Fetch,
            });

            Analysis {
                transformed_js: runtime
                    .render_transformed(db, file.module(db), "playground:runtime".into())
                    .clone(),
                rust_glue: runtime.render_rs_ffi(db, &backend),
                js_glue: runtime.render_js_ffi(db, &backend, &"./playground.wasm".into()),
                diagnostics: diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.render(Theme::UnicodeNoColor).unwrap())
                    .collect::<Vec<_>>()
                    .join("\n"),
            }
        })
    }
}
