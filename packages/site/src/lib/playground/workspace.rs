use ra_ap_cfg::CfgOptions;
use ra_ap_hir::{ChangeWithProcMacros, Symbol, attach_db};
use ra_ap_ide::{
    AnalysisHost, AssistResolveStrategy, DiagnosticsConfig, FilePosition, FileRange,
    GotoDefinitionConfig, HoverConfig, HoverDocFormat, SubstTyLen, TextRange, TextSize,
};
use ra_ap_ide_db::{
    RootDatabase, Severity,
    base_db::{
        AbsPathBuf, CrateDisplayName, CrateGraphBuilder, CrateName, CrateOrigin,
        CrateWorkspaceData, DependencyBuilder, Env, FileId, FileSet, SourceRoot, VfsPath,
        target::{Arch, TargetData},
    },
    ra_fixture::RaFixtureConfig,
    span::Edition,
};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use ra_ap_paths::Utf8PathBuf;
use rustier::compiler::{
    codegen::{js::JsCodegenPaths, shim::Platform},
    diag::Theme,
    inputs::LocalCrates,
    ir::rust::{Rust, usages::UsageGraph},
    path::FilePath,
};
use triomphe::Arc;

include!(concat!(env!("OUT_DIR"), "/sysroot.rs"));

pub struct Workspace {
    host: AnalysisHost,
    source: String,
}

impl Workspace {
    pub fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        console_error_panic_hook::set_once();

        #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
        let unused_proc_macro_cwd = Arc::new(unsafe {
            std::mem::transmute::<Utf8PathBuf, AbsPathBuf>(Utf8PathBuf::from("/playground"))
        });
        #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
        let unused_proc_macro_cwd =
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
            unused_proc_macro_cwd.clone(),
            workspace_data.clone(),
        );
        let mut libraries = Vec::new();

        for (index, &(path, text)) in SYSROOT.iter().enumerate() {
            let file_id = FileId::from_raw(index as u32 + 1);
            let path = format!("/sysroot/{path}");

            library_files.insert(file_id, VfsPath::new_virtual_path(path.clone()));
            change.change_file(file_id, Some(text.to_owned()));

            if let Some(name) = path
                .strip_prefix("/sysroot/")
                .and_then(|path| path.strip_suffix("/src/lib.rs"))
            {
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
                    unused_proc_macro_cwd.clone(),
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
            source: String::new(),
        }
    }

    pub fn compile(&mut self, rust_code: &str) -> Compilation {
        self.update_source(rust_code);

        let config = DiagnosticsConfig {
            proc_macros_enabled: false,
            proc_attr_macros_enabled: false,
            style_lints: false,
            snippet_cap: None,
            term_search_fuel: 0,
            term_search_borrowck: false,
            ..DiagnosticsConfig::test_sample()
        };
        let rust_markers: Vec<_> = self
            .host
            .analysis()
            .full_diagnostics(&config, AssistResolveStrategy::None, FileId::from_raw(0))
            .unwrap_or_default()
            .into_iter()
            .filter(|diagnostic| diagnostic.range.file_id == FileId::from_raw(0))
            .map(|diagnostic| {
                let (start, end) = editor_offsets(rust_code, diagnostic.range.range);

                EditorMarker {
                    start,
                    end,
                    severity: match diagnostic.severity {
                        Severity::Error => 8,
                        Severity::Warning => 4,
                        Severity::WeakWarning => 2,
                        Severity::Allow => 1,
                    },
                    message: diagnostic.message,
                    code: diagnostic.code.as_str().to_owned(),
                }
            })
            .collect();
        let mut analyzer_report = String::new();

        for marker in &rust_markers {
            analyzer_report.push_str(if marker.severity == 8 {
                "\x1b[31m"
            } else {
                "\x1b[33m"
            });
            analyzer_report.push_str(&marker.code);
            analyzer_report.push_str("\x1b[0m: ");
            analyzer_report.push_str(&marker.message);
            analyzer_report.push('\n');
        }

        if rust_markers.iter().any(|marker| marker.severity == 8) {
            return Compilation {
                diagnostics: analyzer_report,
                rust_markers: EditorMarkers {
                    items: rust_markers,
                },
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
            let shim = rust.shim_reachable_from(db, UsageGraph::new(db, [source.root(db)]));
            let module = source.root(db).shim_reachable_from(db, shim.usages(db));

            let mut ts_code = module.decals(db).render_ts_mod(db).clone();
            ts_code.push_str("\n//# sourceMappingURL=lib.rs.d.ts.map\n");

            let mut diagnostics = analyzer_report;

            for diagnostic in shim.check(db) {
                if !diagnostics.is_empty() {
                    diagnostics.push('\n');
                }

                diagnostics.push_str(
                    &diagnostic
                        .render(Theme::UnicodeAnsi)
                        .expect("compiler diagnostics must render into a string"),
                );
            }

            Compilation {
                js_code: module
                    .facade(db)
                    .render(
                        db,
                        JsCodegenPaths {
                            ffi_js_mod_id: "$rust/unstable!/wasm".to_owned(),
                        },
                    )
                    .clone(),
                rust_code: shim.render(db, Platform::Wasm).clone(),
                ts_code,
                ambient_ts_code: shim.decals(db).render_ambient_ts_mods(db),
                ts_sourcemap: module.decals(db).render_sourcemap(db).clone(),
                diagnostics,
                rust_markers: EditorMarkers {
                    items: rust_markers,
                },
            }
        })
    }

    pub fn hover(&mut self, rust_code: &str, offset: u32) -> EditorHover {
        self.update_source(rust_code);

        let config = HoverConfig {
            links_in_hover: false,
            memory_layout: None,
            documentation: true,
            keywords: true,
            format: HoverDocFormat::Markdown,
            max_trait_assoc_items_count: Some(5),
            max_fields_count: Some(5),
            max_enum_variants_count: Some(5),
            max_subst_ty_len: SubstTyLen::LimitTo(128),
            show_drop_glue: false,
            ra_fixture: RaFixtureConfig {
                disable_ra_fixture: true,
                ..RaFixtureConfig::default()
            },
        };
        let position = self.byte_offset(offset);
        let result = self.host.analysis().hover(
            &config,
            FileRange {
                file_id: FileId::from_raw(0),
                range: TextRange::empty(position),
            },
        );

        let Some(result) = result.ok().flatten() else {
            return EditorHover::default();
        };
        let (start, end) = editor_offsets(rust_code, result.range);

        EditorHover {
            start,
            end,
            markdown: result.info.markup.as_str().to_owned(),
        }
    }

    pub fn definition(&mut self, rust_code: &str, offset: u32) -> EditorDefinition {
        self.update_source(rust_code);

        let config = GotoDefinitionConfig {
            ra_fixture: RaFixtureConfig {
                disable_ra_fixture: true,
                ..RaFixtureConfig::default()
            },
        };
        let result = self.host.analysis().goto_definition(
            FilePosition {
                file_id: FileId::from_raw(0),
                offset: self.byte_offset(offset),
            },
            &config,
        );

        let Some(target) = result.ok().flatten().and_then(|result| {
            result
                .info
                .into_iter()
                .find(|target| target.file_id == FileId::from_raw(0))
        }) else {
            return EditorDefinition::default();
        };
        let (start, end) =
            editor_offsets(rust_code, target.focus_range.unwrap_or(target.full_range));

        EditorDefinition {
            found: true,
            start,
            end,
        }
    }

    fn update_source(&mut self, rust_code: &str) {
        if self.source == rust_code {
            return;
        }

        let mut change = ChangeWithProcMacros::default();

        change.change_file(FileId::from_raw(0), Some(rust_code.to_owned()));
        change.apply(self.host.raw_database_mut());
        self.source.clear();
        self.source.push_str(rust_code);
    }

    fn byte_offset(&self, offset: u32) -> TextSize {
        let mut utf16 = 0;
        let mut byte = self.source.len();

        for (index, character) in self.source.char_indices() {
            if utf16 >= offset as usize {
                byte = index;
                break;
            }

            utf16 += character.len_utf16();
        }

        TextSize::from(byte as u32)
    }
}

#[derive(Clone, Default)]
pub struct Compilation {
    pub js_code: String,
    pub rust_code: String,
    pub ts_code: String,
    pub ambient_ts_code: String,
    pub ts_sourcemap: String,
    pub diagnostics: String,
    pub rust_markers: EditorMarkers,
}

#[derive(Clone, Default)]
pub struct EditorMarkers {
    items: Vec<EditorMarker>,
}

impl EditorMarkers {
    pub fn count(&self) -> u32 {
        self.items.len() as u32
    }

    pub fn take(&mut self) -> EditorMarker {
        self.items.pop().unwrap_or_default()
    }
}

#[derive(Clone, Default)]
pub struct EditorMarker {
    pub start: u32,
    pub end: u32,
    pub severity: u32,
    pub message: String,
    pub code: String,
}

#[derive(Clone, Default)]
pub struct EditorHover {
    pub start: u32,
    pub end: u32,
    pub markdown: String,
}

#[derive(Clone, Default)]
pub struct EditorDefinition {
    pub found: bool,
    pub start: u32,
    pub end: u32,
}

fn editor_offsets(source: &str, range: TextRange) -> (u32, u32) {
    let start = usize::from(range.start()).min(source.len());
    let end = usize::from(range.end()).min(source.len());

    let before_start = &source[..start];
    let before_end = &source[..end];

    let start = before_start.encode_utf16().count() as u32;
    let end = before_end.encode_utf16().count() as u32;

    (start, end)
}
