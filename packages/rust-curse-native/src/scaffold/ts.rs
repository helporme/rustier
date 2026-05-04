use std::fs;
use std::path::PathBuf;

use crate::ir::ts::PrintTsModOpts;
use crate::ir::{Id, Item, IR};

use super::{Scaffold, ScaffoldError, ScaffoldSection};

pub const TS_FILE: &str = "ambient.d.ts";
pub const TS_TYPES_DIR: &str = "types";

impl Scaffold {
    pub fn set_ts(&mut self, ir: &IR) -> Result<(), ScaffoldError> {
        let section = ScaffoldSection::new("ts")
            .external(TS_FILE)
            .external(&format!("{TS_TYPES_DIR}/"));
        self.set(section)?;

        let abs_ambient = self.abs_path_to(TS_FILE);
        if let Some(parent) = abs_ambient.parent() {
            fs::create_dir_all(parent).map_err(|e| ScaffoldError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        fs::write(&abs_ambient, ir.print_ts_ambient()).map_err(|e| ScaffoldError::Io {
            path: abs_ambient,
            source: e,
        })?;

        let abs_types_dir = self.abs_path_to(TS_TYPES_DIR);
        if abs_types_dir.exists() {
            fs::remove_dir_all(&abs_types_dir).map_err(|e| ScaffoldError::Io {
                path: abs_types_dir.clone(),
                source: e,
            })?;
        }

        let rel_src_path = PathBuf::from("src");
        for (i, item) in ir.items.iter().enumerate() {
            if !matches!(item, Item::Mod(_)) {
                continue;
            }
            let mid = Id(i as u32);
            let segs = &ir.get_mod_unchecked(mid).path.segments;
            if segs.len() < 2 {
                continue;
            }
            let mut rel_path = rel_src_path.clone();
            for seg in &segs[1..segs.len() - 1] {
                rel_path.push(seg);
            }
            rel_path.push(format!("{}.rs.d.ts", segs.last().unwrap()));

            let content = ir.print_ts_mod(PrintTsModOpts {
                mod_id: mid,
                rel_src_path: &rel_src_path,
            });

            let abs_path = abs_types_dir.join(&rel_path);
            if let Some(parent) = abs_path.parent() {
                fs::create_dir_all(parent).map_err(|e| ScaffoldError::Io {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
            fs::write(&abs_path, &content).map_err(|e| ScaffoldError::Io {
                path: abs_path,
                source: e,
            })?;
        }

        Ok(())
    }
}
