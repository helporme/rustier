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
    ///     change.set_local_file(root.clone(), Some("pub fn greet() {}".into()));
    ///
    ///     let mut crates = CrateGraphBuilder::default();
    ///     crates.add_crate(WorkspaceCrate {
    ///         name: "example".into(),
    ///         edition: Edition::E2024,
    ///         root_file: root,
    ///         version: "0.1.0".into(),
    ///         is_local: true,
    ///         is_sysroot: false,
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
    /// Stable identifier for this diagnostic, when one is defined.
    pub code: Option<String>,
    /// Primary diagnostic message.
    pub message: String,
    /// Primary byte-offset span in the diagnostic source.
    pub span: Option<CompilerDiagnosticSpan>,
    /// Diagnostic severity. Diagnostics without an explicit severity are
    /// errors.
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
