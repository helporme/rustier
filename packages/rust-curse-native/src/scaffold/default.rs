//! Default fallback section: active when no cargo feature and no env
//! flag are set, primarily so rust-analyzer has something to load.

use super::{Scaffold, ScaffoldError, ScaffoldSection};

pub struct DefaultSectionOpts {
    pub lib_content: String,
}

impl Scaffold {
    pub fn set_default(&mut self, opts: DefaultSectionOpts) -> Result<(), ScaffoldError> {
        let section = ScaffoldSection::new("default")
            .cargo_feature_default()
            .lib(opts.lib_content);

        self.set(section)
    }
}
