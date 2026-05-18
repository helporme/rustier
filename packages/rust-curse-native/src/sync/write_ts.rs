use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::codegen::ir::{Ir, IrId, Item};
use crate::codegen::ts::PrintTsModOpts;

use super::Syncer;

pub const TS_AMBIENT_FILE: &str = "ambient.d.ts";
pub const TS_TYPES_DIR: &str = "types";

/// Error returned by [`Syncer::write_ts`] when creating directories or
/// writing files fails.
#[derive(Debug, thiserror::Error)]
pub enum WriteTsError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to create output directory: {0}")]
    CreateOutDir(#[source] io::Error),
}

impl Syncer {
    /// Writes the ambient `$rust-curse` preamble to `<out>/ambient.d.ts`
    /// and one `<out>/types/<seg1>/.../<segN>.rs.d.ts` body per non-root
    /// module in `ir`.
    pub fn write_ts(&mut self, ir: &Ir, modules_root_rel_path: &Path) -> Result<(), WriteTsError> {
        let abs_out_dir = self.abs_out_dir();
        fs::create_dir_all(&abs_out_dir).map_err(WriteTsError::CreateOutDir)?;

        let abs_ambient = abs_out_dir.join(TS_AMBIENT_FILE);
        fs::write(&abs_ambient, ir.print_ts_ambient()).map_err(|e| WriteTsError::Io {
            path: abs_ambient,
            source: e,
        })?;

        let abs_types_dir = abs_out_dir.join(TS_TYPES_DIR);
        if abs_types_dir.exists() {
            fs::remove_dir_all(&abs_types_dir).map_err(|e| WriteTsError::Io {
                path: abs_types_dir.clone(),
                source: e,
            })?;
        }

        for (i, item) in ir.items.iter().enumerate() {
            if !matches!(item, Item::Mod(_)) {
                continue;
            }
            let mid = IrId(i as u32);
            let segs = &ir.get_mod_unchecked(mid).path.segments;
            if segs.len() < 2 {
                continue;
            }

            let mut rel_dts = PathBuf::from(&modules_root_rel_path);
            for seg in &segs[1..segs.len() - 1] {
                rel_dts.push(seg);
            }
            rel_dts.push(format!("{}.rs.d.ts", segs.last().unwrap()));

            let body = ir.print_ts_mod(PrintTsModOpts {
                mod_id: mid,
                rel_src_path: modules_root_rel_path,
            });

            let abs_dts = abs_types_dir.join(&rel_dts);
            if let Some(parent) = abs_dts.parent() {
                fs::create_dir_all(parent).map_err(|e| WriteTsError::Io {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
            fs::write(&abs_dts, body).map_err(|e| WriteTsError::Io {
                path: abs_dts,
                source: e,
            })?;
        }

        Ok(())
    }
}
