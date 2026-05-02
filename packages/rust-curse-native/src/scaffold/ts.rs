use std::fs;

use super::{Scaffold, ScaffoldError, ScaffoldSection};

pub const TS_FILE: &str = "ambient.d.ts";

pub struct TsSectionOpts<'a> {
    pub content: &'a str,
}

impl Scaffold {
    pub fn set_ts(&mut self, opts: TsSectionOpts<'_>) -> Result<(), ScaffoldError> {
        let section = ScaffoldSection::new("ts").external(TS_FILE);
        self.set(section)?;

        let abs_path = self.abs_path_to(TS_FILE);
        if let Some(parent) = abs_path.parent() {
            fs::create_dir_all(parent).map_err(|e| ScaffoldError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        fs::write(&abs_path, opts.content).map_err(|e| ScaffoldError::Io {
            path: abs_path,
            source: e,
        })?;

        Ok(())
    }
}
