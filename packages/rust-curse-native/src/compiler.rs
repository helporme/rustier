mod change;
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

use std::collections::HashMap;

use miette::{GraphicalReportHandler, GraphicalTheme};
use ra_ap_vfs::Vfs;

use self::{
    db::RootDatabase,
    diag::Diag,
    inputs::CratesMetadata,
    ir::{rust::Rust, shim::ShimMod},
};

pub use self::{
    change::{
        CrateBuilderId, CrateGraphBuilder, Edition, WorkspaceChange, WorkspaceChangeError,
        WorkspaceCrate, WorkspaceCrateDependency,
    },
    inputs::{CrateApiDiscovery, CrateApiImportMode, FfiLayout, Platform, WasmEnv},
    path::{FilePath, FilePathError},
};

#[derive(Clone, Copy)]
pub enum Theme {
    Unicode,
    UnicodeNoColor,
}

/// An incremental rust-curse compiler.
pub struct Compiler {
    db: RootDatabase,
    vfs: Vfs,
}

impl Compiler {
    pub fn new() -> Self {
        let mut compiler = Self {
            db: RootDatabase::default(),
            vfs: Vfs::default(),
        };

        CratesMetadata::set(
            &mut compiler.db,
            CratesMetadata {
                crates: HashMap::new(),
            },
        );

        compiler
    }

    /// Changes the workspace (source files, crates) available to the compiler.
    ///
    /// # Example:
    /// ```ignore
    /// use rust_curse::compiler::{
    ///     Compiler, CrateApiDiscovery, CrateApiImportMode, CrateGraphBuilder,
    ///     Edition, FilePath, WorkspaceChangeError, WorkspaceCrate,
    /// };
    ///
    /// let mut compiler = Compiler::new();
    ///
    /// compiler.change_workspace(|change| {
    ///     let root = FilePath::new_virtual("/src/lib.rs")?;
    ///     change.set_local_file(root.clone(), Some("pub fn greet() {}".into()));
    ///
    ///     let mut crates = CrateGraphBuilder::default();
    ///     crates.add_crate(WorkspaceCrate {
    ///         name: "example".into(),
    ///         edition: Edition::E2024,
    ///         root_file: root,
    ///         version: "0.1.0".into(),
    ///         is_local: true,
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
        let mut change = WorkspaceChange::new();
        once(&mut change)?;
        change.apply(&mut self.db, &mut self.vfs)
    }

    /// Compiles a shim code and collects diagnostics.
    pub fn compile_shim(&mut self, opts: CompileShimOpts) -> ShimCompilationOutput {
        Platform::set(&mut self.db, opts.platform);
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
    pub fn compile_js(&mut self, path: FilePath, opts: CompileJsOpts) -> Option<String> {
        FfiLayout::set(
            &mut self.db,
            FfiLayout {
                ffi_js_file: opts.ffi_js_file,
            },
        );
        self.shim_mod(path)
            .map(|mod_| mod_.facade(&self.db).render(&self.db).to_owned())
    }

    /// Compiles the TypeScript declaration tree.
    pub fn compile_ts_tree(&self) -> TsTreeCompilationOutput<'_> {
        let db = &self.db;
        let mut modules = Rust::get(db)
            .shim(db)
            .crates(db)
            .iter()
            .flat_map(|crate_| crate_.mods(db))
            .map(|mod_| {
                let decals = mod_.decals(db);
                TsModuleCompilationOutput {
                    source_path: decals.file_path(db),
                    declaration: decals.render_ts(db),
                    sourcemap: decals.render_sourcemap(db),
                }
            })
            .collect::<Vec<_>>();
        modules.sort_by(|left, right| left.source_path.as_path().cmp(right.source_path.as_path()));

        // inline Rust modules share their containing module's source file and
        // therefore do not have a distinct declaration-tree output.
        modules.dedup_by(|left, right| left.source_path == right.source_path);

        TsTreeCompilationOutput { modules }
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

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileShimOpts {
    /// Target platform used for FFI.
    pub platform: Platform,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileJsOpts {
    /// JavaScript file exposing the generated FFI functions.
    pub ffi_js_file: FilePath,
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

#[derive(Debug)]
pub struct TsTreeCompilationOutput<'compiler> {
    /// TypeScript declaration output for every compiled Rust module.
    pub modules: Vec<TsModuleCompilationOutput<'compiler>>,
}

#[derive(Debug, Clone, Copy)]
pub struct TsModuleCompilationOutput<'compiler> {
    /// Rust source file represented by this declaration module.
    pub source_path: &'compiler FilePath,
    /// Rendered TypeScript declarations.
    pub declaration: &'compiler str,
    /// Source Map v3 document for the declarations.
    pub sourcemap: &'compiler str,
}

#[derive(Clone)]
pub struct CompilerDiagnostic {
    /// Stable identifier for this diagnostic, when one is defined.
    pub code: Option<String>,
    /// Primary diagnostic message.
    pub message: String,
    /// Primary byte-offset span in the diagnostic source.
    pub span: Option<CompilerDiagnosticSpan>,
    /// Diagnostic severity.
    pub severity: CompilerDiagnosticSeverity,
    /// Suggested remediation, when one is available.
    pub help_text: Option<String>,
    inner: Diag,
}

impl CompilerDiagnostic {
    pub fn render(&self, theme: Theme) -> Result<String, std::fmt::Error> {
        let handler = match theme {
            Theme::Unicode => GraphicalReportHandler::new(),
            Theme::UnicodeNoColor => {
                GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
            }
        }
        .with_context_lines(0)
        .with_show_related_as_nested(true);

        let mut rendered = String::new();
        handler.render_report(&mut rendered, self.inner.inner.as_ref())?;

        Ok(rendered)
    }
}

impl From<Diag> for CompilerDiagnostic {
    fn from(inner: Diag) -> Self {
        let code = inner.inner.code().map(|code| code.to_string());
        let message = inner.inner.to_string();
        let span = inner
            .inner
            .labels()
            .and_then(|mut labels| labels.next())
            .map(|label| CompilerDiagnosticSpan {
                offset: label.offset(),
                length: label.len(),
            });
        let severity = match inner.inner.severity().unwrap_or(miette::Severity::Error) {
            miette::Severity::Advice => CompilerDiagnosticSeverity::Advice,
            miette::Severity::Warning => CompilerDiagnosticSeverity::Warning,
            miette::Severity::Error => CompilerDiagnosticSeverity::Error,
        };
        let help_text = inner.inner.help().map(|help| help.to_string());

        Self {
            code,
            message,
            span,
            severity,
            help_text,
            inner,
        }
    }
}

impl std::fmt::Debug for CompilerDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompilerDiagnostic")
            .field("code", &self.code)
            .field("message", &self.message)
            .field("span", &self.span)
            .field("severity", &self.severity)
            .field("help_text", &self.help_text)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompilerDiagnosticSpan {
    pub offset: usize,
    pub length: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CompilerDiagnosticSeverity {
    Advice,
    Warning,
    #[default]
    Error,
}

#[cfg(test)]
mod api_tests {
    use ra_ap_vfs::Vfs;

    use super::Compiler;

    #[test]
    fn compile_ts_tree_returns_borrowed_output_for_every_module() {
        let db = crate::test_db! {
            @file: "models.rs";
            pub mod models {
                #[derive(Clone)]
                pub struct User {
                    pub id: i32,
                }
            }

            pub mod inline {
                pub struct Inline;
            }

            pub fn user_id(user: models::User) -> i32 {
                user.id
            }
        };
        let compiler = Compiler {
            db,
            vfs: Vfs::default(),
        };

        let output = compiler.compile_ts_tree();

        assert_eq!(output.modules.len(), 2);
        assert_eq!(
            output.modules[0].source_path.as_path().to_str(),
            Some("/src/lib.rs")
        );
        assert!(
            output.modules[0]
                .declaration
                .contains("export function user_id")
        );
        assert_eq!(
            output.modules[1].source_path.as_path().to_str(),
            Some("/src/models.rs")
        );
        assert!(output.modules[1].declaration.contains("export class User"));
        for module in output.modules {
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(module.sourcemap)
                    .expect("source map should be valid JSON")["version"],
                3
            );
        }
    }
}
