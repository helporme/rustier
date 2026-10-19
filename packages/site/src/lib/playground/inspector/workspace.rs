use ra_ap_cfg::CfgOptions;
use ra_ap_hir::{ChangeWithProcMacros, Symbol, attach_db};
use ra_ap_ide::{AnalysisHost, AssistResolveStrategy, DiagnosticsConfig};
use ra_ap_ide_db::{
    RootDatabase, Severity,
    base_db::{
        AbsPathBuf, CrateDisplayName, CrateGraphBuilder, CrateName, CrateOrigin,
        CrateWorkspaceData, DependencyBuilder, Env, FileId, FileSet, SourceRoot, VfsPath,
        target::{Arch, TargetData},
    },
    span::Edition,
};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use ra_ap_paths::Utf8PathBuf;
use rustier::compiler::{
    codegen::{
        abi::js::{AbiJsTarget, WasmLoading},
        js::JsCodegenPaths,
    },
    diag::Theme,
    inputs::LocalCrates,
    ir::{
        abi::transport::{AbiTransport, WasmMemory, WasmTransport},
        rust::{Rust, usages::UsageGraph},
    },
    path::FilePath,
};
use triomphe::Arc;

include!(concat!(env!("OUT_DIR"), "/sysroot.rs"));

pub struct Workspace {
    host: AnalysisHost,
}

impl Workspace {
    pub fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        console_error_panic_hook::set_once();

        // wasm32-unknown-unknown cannot validate absolute paths. This is only the
        // virtual cwd required by rust-analyzer; proc macros are disabled here.
        #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
        let cwd = Arc::new(unsafe {
            std::mem::transmute::<Utf8PathBuf, AbsPathBuf>(Utf8PathBuf::from("/playground"))
        });
        #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
        let cwd =
            Arc::new(AbsPathBuf::try_from("/playground").expect("workspace path must be absolute"));

        let mut db = RootDatabase::default();
        let mut change = ChangeWithProcMacros::default();
        let mut local_files = FileSet::default();
        let mut library_files = FileSet::default();
        let mut graph = CrateGraphBuilder::default();
        let workspace_data = Arc::new(CrateWorkspaceData {
            target: Ok(TargetData {
                data_layout: "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
                    .into(),
                arch: Arch::Wasm32,
            }),
            toolchain: Some(
                env!("RUSTIER_RUST_VERSION")
                    .parse()
                    .expect("rustc version must be valid"),
            ),
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
        local_files.insert(
            FileId::from_raw(0),
            VfsPath::new_virtual_path("/playground/lib.rs".into()),
        );
        change.change_file(FileId::from_raw(0), Some(String::new()));

        let source = graph.add_crate_root(
            FileId::from_raw(0),
            Edition::Edition2024,
            Some(CrateDisplayName::from_canonical_name("playground")),
            None,
            cfg.clone(),
            None,
            Env::default(),
            CrateOrigin::Local {
                repo: None,
                name: None,
            },
            Vec::new(),
            false,
            cwd.clone(),
            workspace_data.clone(),
        );
        let mut libraries = Vec::new();

        for (index, &(path, text)) in SYSROOT.iter().enumerate() {
            let file_id = FileId::from_raw(index as u32 + 1);

            library_files.insert(
                file_id,
                VfsPath::new_virtual_path(format!("/sysroot/{path}")),
            );
            change.change_file(file_id, Some(text.to_owned()));

            if let Some(name) = path.strip_suffix("/src/lib.rs") {
                let crate_ = graph.add_crate_root(
                    file_id,
                    Edition::Edition2024,
                    Some(CrateDisplayName::from_canonical_name(name)),
                    None,
                    cfg.clone(),
                    None,
                    Env::default(),
                    CrateOrigin::Lang(name.into()),
                    Vec::new(),
                    false,
                    cwd.clone(),
                    workspace_data.clone(),
                );

                libraries.push((name.to_owned(), crate_));
            }
        }

        for &(ref name, crate_) in &libraries {
            graph
                .add_dep(
                    source,
                    DependencyBuilder::new(
                        CrateName::new(name).expect("sysroot names must be valid"),
                        crate_,
                    ),
                )
                .expect("source dependencies must be acyclic");

            for &(ref dependency, target) in &libraries {
                if (name == "alloc" && dependency == "core")
                    || (name == "std" && dependency != "std")
                {
                    graph
                        .add_dep(
                            crate_,
                            DependencyBuilder::new(
                                CrateName::new(dependency).expect("sysroot names must be valid"),
                                target,
                            ),
                        )
                        .expect("sysroot dependencies must be acyclic");
                }
            }
        }

        change.set_roots(vec![
            SourceRoot::new_local(local_files),
            SourceRoot::new_library(library_files),
        ]);
        change.set_crate_graph(graph);
        change.apply(&mut db);
        LocalCrates::set(
            &mut db,
            [FilePath::new_virtual("/playground/lib.rs").expect("workspace path must be valid")],
        );

        Self {
            host: AnalysisHost::with_database(db),
        }
    }

    pub fn compile(&mut self, code: &str) -> Compilation {
        let mut change = ChangeWithProcMacros::default();

        change.change_file(FileId::from_raw(0), Some(code.to_owned()));
        change.apply(self.host.raw_database_mut());

        let config = DiagnosticsConfig {
            proc_macros_enabled: false,
            proc_attr_macros_enabled: false,
            style_lints: false,
            snippet_cap: None,
            term_search_fuel: 0,
            term_search_borrowck: false,
            ..DiagnosticsConfig::test_sample()
        };
        let diagnostics = self
            .host
            .analysis()
            .full_diagnostics(&config, AssistResolveStrategy::None, FileId::from_raw(0))
            .unwrap_or_default();
        let mut report = String::new();
        let mut diagnostic_count = 0;
        let mut has_errors = false;

        for diagnostic in diagnostics {
            if diagnostic.range.file_id != FileId::from_raw(0) {
                continue;
            }

            diagnostic_count += 1;
            has_errors |= diagnostic.severity == Severity::Error;
            report.push_str(diagnostic.code.as_str());
            report.push_str(": ");
            report.push_str(&diagnostic.message);
            report.push('\n');
        }

        if has_errors {
            return Compilation {
                diagnostics: report,
                diagnostic_count,
                ..Compilation::default()
            };
        }

        let db = self.host.raw_database();

        attach_db(db, || {
            let rust = Rust::get(db);
            let source = rust
                .crates(db)
                .iter()
                .find(|crate_| crate_.is_local(db))
                .expect("workspace must contain its source crate");
            let facade = rust.facade_reachable_from(db, UsageGraph::new(db, [source.root(db)]));
            let module = source.root(db).facade_reachable_from(db, facade.usages(db));
            let runtime = facade.runtime(db);

            for diagnostic in facade.check(db) {
                diagnostic_count += 1;
                report.push_str(
                    &diagnostic
                        .render(Theme::UnicodeNoColor)
                        .expect("compiler diagnostics must render into a string"),
                );
                report.push('\n');
            }

            Compilation {
                js_code: module
                    .render(
                        db,
                        JsCodegenPaths {
                            abi_js_mod_id: "$rust/unstable!/wasm".to_owned(),
                        },
                    )
                    .clone(),
                rust_code: runtime
                    .render_rust(
                        db,
                        AbiTransport::Wasm(WasmTransport {
                            memory: WasmMemory::x32,
                        }),
                    )
                    .clone(),
                ts_code: module.render_ts_mod(db).clone(),
                ambient_ts_code: facade.render_ambient_ts_mods(db),
                runtime_js_code: runtime.render_js(
                    db,
                    AbiJsTarget::Wasm {
                        loading: WasmLoading::Fetch,
                        path: "./playground.js",
                        wasm_path: "./playground_bg.wasm",
                    },
                ),
                diagnostics: report,
                diagnostic_count,
            }
        })
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Default)]
pub struct Compilation {
    pub js_code: String,
    pub rust_code: String,
    pub ts_code: String,
    pub ambient_ts_code: String,
    pub runtime_js_code: String,
    pub diagnostics: String,
    pub diagnostic_count: u32,
}
