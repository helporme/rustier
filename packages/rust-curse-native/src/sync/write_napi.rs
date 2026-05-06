use crate::sync::patch_cargo_toml::CargoTomlPatch;

use super::patch_cargo_toml::{patch_cargo_toml_file, Dep};
use super::write_build::{write_build, BuildBranch};
use super::write_lib::{write_lib, LibBranch};
use super::Syncer;

pub const NAPI_CARGO_FEATURE: &str = "node";

impl Syncer {
    /// Writes NAPI glue code to the `lib.rs`, `build.rs` and patches the
    /// `Cargo.toml` file.
    ///
    /// `lib.rs` and `build.rs` content is combined with previous
    /// `Syncer::write_*` calls if any.
    pub fn write_napi(&mut self, napi_glue_code: String) {
        patch_cargo_toml_file(
            &self.project_root_abs_path.join("Cargo.toml"),
            &CargoTomlPatch::new()
                .dep(Dep::new("wasm-bindgen", "0.2").requires_feature(NAPI_CARGO_FEATURE))
                .dep(Dep::new("serde-wasm-bindgen", "0.6").requires_feature(NAPI_CARGO_FEATURE))
                .dep(
                    Dep::new("serde", "1")
                        .feature("derive")
                        .requires_feature(NAPI_CARGO_FEATURE),
                ),
        )
        .unwrap();

        self.lib.set_branch(
            "napi",
            LibBranch::new(napi_glue_code).requires_cargo_feature(NAPI_CARGO_FEATURE),
        );
        write_lib(&self.project_root_abs_path, &self.lib).unwrap();

        self.build.set_branch(
            "napi",
            BuildBranch::new("use napi_build;\nnapi_build::setup();")
                .requires_cargo_feature(NAPI_CARGO_FEATURE),
        );
        write_build(&self.project_root_abs_path, &self.build).unwrap();
    }
}
