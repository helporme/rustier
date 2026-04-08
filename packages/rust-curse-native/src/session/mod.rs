use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use napi_derive::napi;

use crate::codegen::adoption;
use crate::discovery::orphans::discover_orphans;
use crate::model::{GeneratedFile, OrphanModule};

#[napi(string_enum)]
pub enum BuildTarget {
    Prelude,
    Napi,
    Wasm,
}

#[napi(object)]
pub struct SourceOptions {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[napi(object)]
pub struct BuildOptions {
    pub target: BuildTarget,
    pub js_source: SourceOptions,
    /// Directory to discover rust orphan modules.
    pub module_dir: String,
    /// Output dir for placing generated files e.g. .rust-curse/prelude.
    pub output_dir: String,
    /// Path to the original Cargo.toml.
    pub cargo_path: String,
    /// Project root directory.
    pub project_root: String,
}

#[napi]
pub struct Session {
    options: BuildOptions,
    last_modules: Vec<OrphanModule>,
}

#[napi]
impl Session {
    #[napi(constructor)]
    pub fn new(options: BuildOptions) -> Self {
        Self {
            options,
            last_modules: Vec::new(),
        }
    }

    #[napi]
    pub fn build(&mut self) {
        self.try_build().expect("build failed");
    }

    fn try_build(&mut self) -> anyhow::Result<()> {
        // Step 1: Discover orphan modules
        let modules = discover_orphans(&PathBuf::from(&self.options.module_dir))?;

        // Step 1b: Generate lib.rs that adopts orphan modules
        let abs_module_dir = fs::canonicalize(&self.options.module_dir)
            .context("module_dir does not exist or is not accessible")?;

        fs::create_dir_all(&self.options.output_dir).context("cannot create output_dir")?;
        let abs_output_dir =
            fs::canonicalize(&self.options.output_dir).context("output_dir is not accessible")?;

        let path_prefix = diff_paths(&abs_module_dir, &abs_output_dir);
        let lib_rs = adoption::generate_lib_rs(&modules, &path_prefix, PathBuf::from("lib.rs"));
        emit_file(&abs_output_dir, &lib_rs)?;

        // TODO: Step 2 — run cargo rustdoc, load JSON
        // TODO: Step 3 — link rustdoc items to orphan modules
        // TODO: Step 4 — import scan
        // TODO: Step 5 — treeshake
        // TODO: Step 6 — codegen + emit

        self.last_modules = modules;
        Ok(())
    }

    #[napi]
    pub fn on_rs_file_changed(&mut self, _abs_path: String) {
        // TODO: re-run adoption + rustdoc + affected codegen
    }

    #[napi]
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
