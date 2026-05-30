use crate::sync::patch_cargo_toml::CargoTomlPatch;

use super::patch_cargo_toml::{patch_cargo_toml_file, Dep, PatchCargoTomlError};
use super::write_lib::{write_lib, LibBranch, WriteLibError};
use super::Syncer;
use tracing::*;

pub const WASM_CARGO_FEATURE: &str = "wasm";

/// Error returned by [`Syncer::write_wasm`] when patching `Cargo.toml`,
/// rendering, or writing fails.
#[derive(Debug, thiserror::Error)]
pub enum WriteWasmError {
    #[error("failed to patch Cargo.toml: {0}")]
    PatchCargoToml(#[from] PatchCargoTomlError),
    #[error("failed to write lib.rs: {0}")]
    WriteLib(#[from] WriteLibError),
}

impl Syncer {
    /// Writes WASM glue code to the `lib.rs` file and patches the `Cargo.toml`
    /// file.
    ///
    /// `lib.rs` content is combined with previous `Syncer::write_*` calls if
    /// any.
    pub fn write_wasm(
        &mut self,
        profile_name: &str,
        wasm_glue_code: String,
    ) -> Result<(), WriteWasmError> {
        patch_cargo_toml_file(
            &self.project_root_abs_path.join("Cargo.toml"),
            &CargoTomlPatch::new()
                .crate_type("cdylib")
                .empty_feature(profile_name)
                .dep(Dep::new("wasm-bindgen", "0.2").requires_feature(WASM_CARGO_FEATURE))
                .dep(Dep::new("js-sys", "0.3").requires_feature(WASM_CARGO_FEATURE)),
        )?;

        self.lib.set_branch(
            profile_name,
            LibBranch::new(wasm_glue_code)
                .requires_cargo_feature(WASM_CARGO_FEATURE)
                .requires_cargo_feature(profile_name),
        );
        write_lib(&self.abs_out_dir(), &self.lib)?;

        debug!(profile = profile_name, "wrote wasm glue");
        Ok(())
    }
}
