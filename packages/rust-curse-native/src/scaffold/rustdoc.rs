use super::{Scaffold, ScaffoldError, ScaffoldSection};

pub const RUSTDOC_ENV_VAR: &str = "RUST_CURSE_RUSTDOC";

pub struct RustdocSectionOpts {
    pub lib_content: String,
}

impl Scaffold {
    pub fn set_rustdoc(&mut self, opts: RustdocSectionOpts) -> Result<(), ScaffoldError> {
        let section = ScaffoldSection::new("rustdoc")
            .env_flag(RUSTDOC_ENV_VAR)
            .lib(opts.lib_content);

        self.set(section)
    }
}
