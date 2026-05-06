use crate::sync::patch_cargo_toml::CargoTomlPatch;

use super::patch_cargo_toml::{patch_cargo_toml_file, Dep};
use super::write_lib::{write_lib, LibBranch};
use super::Syncer;

pub const WASM_CARGO_FEATURE: &str = "wasm";

impl Syncer {
    /// Writes WASM glue code to the `lib.rs` file and patches the `Cargo.toml`
    /// file.
    ///
    /// `lib.rs` content is combined with previous `Syncer::write_*` calls if
    /// any.
    pub fn write_wasm(&mut self, wasm_glue_code: String) {
        patch_cargo_toml_file(
            &self.project_root_abs_path.join("Cargo.toml"),
            &CargoTomlPatch::new()
                .dep(Dep::new("wasm-bindgen", "0.2").requires_feature(WASM_CARGO_FEATURE))
                .dep(Dep::new("serde-wasm-bindgen", "0.6").requires_feature(WASM_CARGO_FEATURE))
                .dep(
                    Dep::new("serde", "1")
                        .feature("derive")
                        .requires_feature(WASM_CARGO_FEATURE),
                ),
        )
        .unwrap();

        self.lib.set_branch(
            "wasm",
            LibBranch::new(wasm_glue_code).requires_cargo_feature(WASM_CARGO_FEATURE),
        );
        write_lib(&self.project_root_abs_path, &self.lib).unwrap();
    }
}
