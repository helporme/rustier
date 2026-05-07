use super::patch_cargo_toml::{patch_cargo_toml_file, CargoTomlPatch, PatchCargoTomlError};
use super::Syncer;

/// Error returned by [`Syncer::write_wired_cargo_toml`] when patching
/// `Cargo.toml` fails.
#[derive(Debug, thiserror::Error)]
pub enum WriteWiredCargoTomlError {
    #[error("failed to patch Cargo.toml: {0}")]
    PatchCargoToml(#[from] PatchCargoTomlError),
}

impl Syncer {
    /// Patches the project's `Cargo.toml` to wire `[lib].path` and
    /// `[package].build` to the generated `lib.rs` and `build.rs` inside
    /// `out_dir_rel_path`.
    pub fn write_wired_cargo_toml(&self) -> Result<(), WriteWiredCargoTomlError> {
        let lib_path = posix_rel_path(&self.out_dir_rel_path.join("lib.rs"));
        let build_path = posix_rel_path(&self.out_dir_rel_path.join("build.rs"));
        patch_cargo_toml_file(
            &self.project_root_abs_path.join("Cargo.toml"),
            &CargoTomlPatch::new()
                .lib_path(lib_path)
                .build_path(build_path)
                .crate_type("rlib"),
        )?;
        Ok(())
    }
}

fn posix_rel_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
