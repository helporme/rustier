use super::{Scaffold, ScaffoldError, ScaffoldSection};

pub struct WasmSectionOpts {
    pub content: String,
}

impl Scaffold {
    pub fn set_wasm(&mut self, opts: WasmSectionOpts) -> Result<(), ScaffoldError> {
        let section = ScaffoldSection::new("wasm")
            .cargo_feature_flag("wasm")
            .content(opts.content)
            .dep("wasm-bindgen", "0.2")
            .dep_with_features("serde", "1", &["derive"])
            .dep("serde-wasm-bindgen", "0.6");

        self.set(section)
    }
}
