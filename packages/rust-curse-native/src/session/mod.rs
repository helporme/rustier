use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
#[cfg(feature = "napi-binding")]
use napi_derive::napi;

use crate::codegen::cargo_toml;
use crate::codegen::rustdoc as rustdoc_codegen;
use crate::codegen::setup;
use crate::discovery::import_scan::scan_imports;
use crate::discovery::orphans::discover_orphans;
use crate::discovery::rustdoc::{generate_rustdoc, link_rustdoc_items};
use crate::discovery::treeshake::{treeshake_modules, TreeshakeWarning};
use crate::model::{GeneratedFile, OrphanModule};

/// Rustdoc cfg flag name, set via build.rs when `RUST_CURSE_RUSTDOC` env var is present.
pub const RUSTDOC_CFG: &str = "rust_curse_rustdoc";
/// Env var that triggers the rustdoc cfg flag in the generated build.rs.
pub const RUSTDOC_ENV_VAR: &str = "RUST_CURSE_RUSTDOC";
/// File name for the rustdoc module.
pub const RUSTDOC_FILE: &str = "rustdoc.rs";

#[cfg_attr(feature = "napi-binding", napi)]
pub enum BuildTarget {
    Default {
        js_source: SourceOptions,
    },
    Node {
        js_source: SourceOptions,
        feature_name: String,
    },
    Wasm {
        js_source: SourceOptions,
        feature_name: String,
    },
}

impl BuildTarget {
    /// Cargo feature name that activates this target, if any.
    pub fn feature_name(&self) -> Option<&str> {
        match self {
            BuildTarget::Default { .. } => None,
            BuildTarget::Node { feature_name, .. } => Some(feature_name),
            BuildTarget::Wasm { feature_name, .. } => Some(feature_name),
        }
    }

    /// File name for the generated module (e.g. `"node.rs"`).
    pub fn file_name(&self) -> &'static str {
        match self {
            BuildTarget::Default { .. } => "default.rs",
            BuildTarget::Node { .. } => "node.rs",
            BuildTarget::Wasm { .. } => "wasm.rs",
        }
    }

    pub fn js_source(&self) -> &SourceOptions {
        match self {
            BuildTarget::Default { js_source, .. }
            | BuildTarget::Node { js_source, .. }
            | BuildTarget::Wasm { js_source, .. } => js_source,
        }
    }
}

#[cfg_attr(feature = "napi-binding", napi(string_enum))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BuildStep {
    Setup,
    Rustdoc,
    Link,
    ImportScan,
    Treeshake,
    Codegen,
}

#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct SourceOptions {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct SessionOptions {
    pub targets: Vec<BuildTarget>,
    /// Directory to discover rust orphan modules, relative to project root.
    pub module_dir_rel_path: String,
    /// Output dir for placing generated files, relative to project root.
    pub output_dir_rel_path: String,
    /// Path to the original Cargo.toml, relative to project root.
    pub cargo_toml_rel_path: String,
    /// Absolute path to the project root directory.
    pub project_root_abs_path: String,
    /// Last step to execute (for testing). If None, runs the full pipeline.
    pub last_step: Option<BuildStep>,
}

#[cfg_attr(feature = "napi-binding", napi)]
pub struct Session {
    options: SessionOptions,
    last_modules: Vec<OrphanModule>,
}

#[cfg_attr(feature = "napi-binding", napi(object))]
pub struct BuildResult {
    pub warnings: Vec<String>,
}

impl BuildResult {
    fn empty() -> Self {
        Self {
            warnings: Vec::new(),
        }
    }

    fn from_warnings(warnings: Vec<TreeshakeWarning>) -> Self {
        Self {
            warnings: warnings.iter().map(|w| w.to_string()).collect(),
        }
    }
}

#[cfg_attr(feature = "napi-binding", napi)]
impl Session {
    #[cfg_attr(feature = "napi-binding", napi(constructor))]
    pub fn new(options: SessionOptions) -> Self {
        Self {
            options,
            last_modules: Vec::new(),
        }
    }

    #[cfg_attr(feature = "napi-binding", napi)]
    pub fn build(&mut self) -> BuildResult {
        self.try_build().expect("build failed")
    }

    fn try_build(&mut self) -> anyhow::Result<BuildResult> {
        let last_step = self.options.last_step;

        // Step 1: Setup -> discover orphans, generate lib.rs + rustdoc.rs, patch Cargo.toml
        let modules = discover_orphans(&PathBuf::from(&self.options.module_dir_rel_path))?;

        let abs_module_dir = fs::canonicalize(&self.options.module_dir_rel_path)
            .context("module_dir does not exist or is not accessible")?;

        fs::create_dir_all(&self.options.output_dir_rel_path)
            .context("cannot create output_dir")?;
        let abs_output_dir = fs::canonicalize(&self.options.output_dir_rel_path)
            .context("output_dir is not accessible")?;

        // Generate feature-gated lib.rs + build.rs
        let lib_rs = setup::generate_lib_rs(&self.options.targets);
        emit_file(&abs_output_dir, &lib_rs)?;

        let build_rs = setup::generate_build_rs(&self.options.targets);
        emit_file(&abs_output_dir, &build_rs)?;

        // Generate rustdoc.rs with pub mod declarations for all orphan modules
        let path_prefix = diff_paths(&abs_module_dir, &abs_output_dir);
        let rustdoc_rs = rustdoc_codegen::generate_rustdoc_rs(
            &modules,
            &path_prefix,
            PathBuf::from(RUSTDOC_FILE),
        );
        emit_file(&abs_output_dir, &rustdoc_rs)?;

        // Patch Cargo.toml with lib.path, build path, features + optional deps
        let abs_cargo_path = fs::canonicalize(&self.options.cargo_toml_rel_path)
            .context("cargo_path does not exist or is not accessible")?;
        let abs_cargo_dir = abs_cargo_path
            .parent()
            .context("cargo_path has no parent directory")?;

        let output_rel_to_cargo = diff_paths(&abs_output_dir, abs_cargo_dir);
        let lib_rs_rel_path = output_rel_to_cargo.join("lib.rs");
        let lib_rs_rel_path = lib_rs_rel_path.to_string_lossy();
        let build_rs_rel_path = output_rel_to_cargo.join("build.rs");
        let build_rs_rel_path = build_rs_rel_path.to_string_lossy();

        let cargo_content =
            fs::read_to_string(&abs_cargo_path).context("cannot read Cargo.toml")?;
        let patched = cargo_toml::patch_cargo_toml(
            &cargo_content,
            &lib_rs_rel_path,
            &build_rs_rel_path,
            &self.options.targets,
            PathBuf::from("Cargo.toml"),
        )
        .context("cannot parse Cargo.toml")?;

        fs::write(&abs_cargo_path, &patched.content)
            .with_context(|| format!("cannot write '{}'", abs_cargo_path.display()))?;

        self.last_modules = modules;

        if last_step == Some(BuildStep::Setup) {
            return Ok(BuildResult::empty());
        }

        // Step 2: Run cargo rustdoc, load JSON
        let crate_ = generate_rustdoc(abs_cargo_dir).context("failed to generate rustdoc")?;

        if last_step == Some(BuildStep::Rustdoc) {
            return Ok(BuildResult::empty());
        }

        // Step 3: Link rustdoc items to orphan modules
        let module_dir_rel_to_cargo = diff_paths(&abs_module_dir, abs_cargo_dir);
        link_rustdoc_items(&crate_, &mut self.last_modules, &module_dir_rel_to_cargo);

        if last_step == Some(BuildStep::Link) {
            return Ok(BuildResult::empty());
        }
        // Step 4: Import scan
        let abs_project_root = PathBuf::from(&self.options.project_root_abs_path);
        let mut all_imports = Vec::new();
        for target in &self.options.targets {
            let js_source = target.js_source();
            let include = make_absolute_globs(&js_source.include, &abs_project_root);
            let exclude = make_absolute_globs(&js_source.exclude, &abs_project_root);
            let target_imports =
                scan_imports(&abs_module_dir, &include, &exclude).context("import scan failed")?;
            all_imports.extend(target_imports);
        }

        if last_step == Some(BuildStep::ImportScan) {
            return Ok(BuildResult::empty());
        }

        // Step 5: Treeshake
        let treeshake_result = treeshake_modules(&self.last_modules, &all_imports, &crate_);
        let warnings = treeshake_result.warnings;

        if last_step == Some(BuildStep::Treeshake) {
            return Ok(BuildResult::from_warnings(warnings));
        }

        // TODO: Step 6 — codegen + emit

        Ok(BuildResult::from_warnings(warnings))
    }

    pub fn last_modules(&self) -> &[OrphanModule] {
        &self.last_modules
    }

    #[cfg_attr(feature = "napi-binding", napi)]
    pub fn on_rs_file_changed(&mut self, _abs_path: String) {
        // TODO: re-run adoption + rustdoc + affected codegen
    }

    #[cfg_attr(feature = "napi-binding", napi)]
    pub fn on_js_file_changed(&mut self, _abs_path: String) {
        // TODO: re-scan imports for changed file + treeshake + codegen
    }
}

/// Compute a relative path from `abs_from` to `abs_to`.
///
/// Both paths must be absolute and canonicalized.
fn diff_paths(abs_to: &Path, abs_from: &Path) -> PathBuf {
    let mut from_iter = abs_from.components().peekable();
    let mut to_iter = abs_to.components().peekable();

    // Skip the common prefix
    while let (Some(a), Some(b)) = (from_iter.peek(), to_iter.peek()) {
        if a != b {
            break;
        }
        from_iter.next();
        to_iter.next();
    }

    // One `..` for each remaining component in `from`
    let mut result = PathBuf::new();
    for _ in from_iter {
        result.push("..");
    }

    // Then append the remaining components of `to`
    for component in to_iter {
        result.push(component);
    }

    result
}

fn make_absolute_globs(patterns: &[String], abs_root: &Path) -> Vec<String> {
    patterns
        .iter()
        .map(|p| {
            let path = Path::new(p);
            if path.is_absolute() {
                p.clone()
            } else {
                abs_root.join(p).to_string_lossy().into_owned()
            }
        })
        .collect()
}

fn emit_file(abs_output_dir: &Path, file: &GeneratedFile) -> anyhow::Result<()> {
    let abs_path = abs_output_dir.join(&file.rel_path);
    if let Some(abs_parent) = abs_path.parent() {
        fs::create_dir_all(abs_parent)
            .with_context(|| format!("cannot create directory '{}'", abs_parent.display()))?;
    }
    fs::write(&abs_path, &file.content)
        .with_context(|| format!("cannot write '{}'", abs_path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_paths_sibling_dirs() {
        let result = diff_paths(Path::new("/a/b/src"), Path::new("/a/b/out"));
        assert_eq!(result, PathBuf::from("../src"));
    }

    #[test]
    fn diff_paths_deeper_output() {
        let result = diff_paths(
            Path::new("/project/src"),
            Path::new("/project/.rust-curse/prelude"),
        );
        assert_eq!(result, PathBuf::from("../../src"));
    }

    #[test]
    fn diff_paths_same_dir() {
        let result = diff_paths(Path::new("/a/b"), Path::new("/a/b"));
        assert_eq!(result, PathBuf::from(""));
    }
}
