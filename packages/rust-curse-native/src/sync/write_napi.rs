use crate::sync::patch_cargo_toml::CargoTomlPatch;

use super::patch_cargo_toml::{patch_cargo_toml_file, Dep, PatchCargoTomlError};
use super::write_build::{write_build, BuildBranch, WriteBuildError};
use super::write_lib::{write_lib, LibBranch, WriteLibError};
use super::Syncer;

pub const NAPI_CARGO_FEATURE: &str = "node";

/// Error returned by [`Syncer::write_napi`] when patching `Cargo.toml`,
/// rendering, or writing fails.
#[derive(Debug, thiserror::Error)]
pub enum WriteNapiError {
    #[error("failed to patch Cargo.toml: {0}")]
    PatchCargoToml(#[from] PatchCargoTomlError),
    #[error("failed to write lib.rs: {0}")]
    WriteLib(#[from] WriteLibError),
    #[error("failed to write build.rs: {0}")]
    WriteBuild(#[from] WriteBuildError),
}

impl Syncer {
    /// Writes NAPI glue code to the `lib.rs`, `build.rs` and patches the
    /// `Cargo.toml` file.
    ///
    /// `lib.rs` and `build.rs` content is combined with previous
    /// `Syncer::write_*` calls if any.
    pub fn write_napi(&mut self, napi_glue_code: String) -> Result<(), WriteNapiError> {
        patch_cargo_toml_file(
            &self.project_root_abs_path.join("Cargo.toml"),
            &CargoTomlPatch::new()
                .dep(Dep::new("napi", "3").requires_feature(NAPI_CARGO_FEATURE))
                .dep(Dep::new("napi-derive", "3").requires_feature(NAPI_CARGO_FEATURE))
                .build_dep(
                    Dep::new("napi-build", "2").requires_feature(NAPI_CARGO_FEATURE),
                ),
        )?;

        let abs_out_dir = self.abs_out_dir();

        self.lib.set_branch(
            "napi",
            LibBranch::new(napi_glue_code).requires_cargo_feature(NAPI_CARGO_FEATURE),
        );
        write_lib(&abs_out_dir, &self.lib)?;

        self.build.set_branch(
            "napi",
            BuildBranch::new("use napi_build;\nnapi_build::setup();")
                .requires_cargo_feature(NAPI_CARGO_FEATURE),
        );
        write_build(&abs_out_dir, &self.build)?;

        Ok(())
    }
}
