use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::codegen::ir::{Ir, Item};
use crate::codegen::source_map::write_source_map;
use crate::codegen::ts::{PrintTsModOpts, PrintedTsMod};

use super::write::{sync_dir, write_if_changed};
use super::Syncer;
use tracing::*;

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
        write_if_changed(&abs_ambient, ir.print_ts_ambient().as_bytes()).map_err(|e| {
            WriteTsError::Io {
                path: abs_ambient,
                source: e,
            }
        })?;

        let abs_types_dir = abs_out_dir.join(TS_TYPES_DIR);
        let mut targets: Vec<(PathBuf, Vec<u8>)> = Vec::new();
        let mut module_count = 0usize;

        for item in &ir.items {
            let Item::Mod(m) = item else {
                continue;
            };
            let mid = m.id;
            let segs = &m.path.path;
            if segs.len() < 2 {
                continue;
            }

            let mut rel_dts = PathBuf::from(&modules_root_rel_path);
            for seg in &segs[1..segs.len() - 1] {
                rel_dts.push(seg);
            }
            rel_dts.push(format!("{}.rs.d.ts", segs.last().unwrap()));

            let PrintedTsMod {
                mut body,
                mappings,
                source_rel_path,
            } = ir.print_ts_mod(PrintTsModOpts {
                mod_id: mid,
                rel_src_path: modules_root_rel_path,
            });

            let rel_dts_map = {
                let mut p = rel_dts.clone();
                let file = p.file_name().unwrap().to_owned();
                p.set_file_name(format!("{}.map", file.to_string_lossy()));
                p
            };
            let map_basename = rel_dts_map
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let dts_basename = rel_dts.file_name().unwrap().to_string_lossy().into_owned();

            if let Some(rel_src) = source_rel_path.as_ref() {
                // The source map's `sources` entries are resolved relative to
                // the `.d.ts.map` file itself, not the project root.
                let abs_src = self.project_root_abs_path.join(rel_src);
                let abs_dts = abs_types_dir.join(&rel_dts);
                let dts_dir = abs_dts.parent().unwrap_or_else(|| Path::new(""));
                let rel_src_from_map = path_relative(dts_dir, &abs_src);
                let sources = vec![to_forward_slash(&rel_src_from_map)];
                let map_json = write_source_map(&dts_basename, &sources, &mappings);

                targets.push((rel_dts_map, map_json.into_bytes()));

                if !body.ends_with('\n') {
                    body.push('\n');
                }
                body.push_str(&format!("//# sourceMappingURL={map_basename}\n"));
            }

            targets.push((rel_dts, body.into_bytes()));
            module_count += 1;
        }

        sync_dir(&abs_types_dir, &targets).map_err(|e| WriteTsError::Io {
            path: abs_types_dir,
            source: e,
        })?;

        debug!(modules = module_count, "wrote ts type files");
        Ok(())
    }
}

/// Compute the path of `to_file` relative to `from_dir`. Assumes both
/// paths are normalized (no `..` components) -> they are, since they're
/// built by joining `project_root` with project-relative segments.
fn path_relative(from_dir: &Path, to_file: &Path) -> PathBuf {
    let from: Vec<&std::ffi::OsStr> = from_dir.iter().collect();
    let to: Vec<&std::ffi::OsStr> = to_file.iter().collect();
    let common = from
        .iter()
        .zip(to.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let mut result = PathBuf::new();
    for _ in 0..(from.len() - common) {
        result.push("..");
    }
    for c in &to[common..] {
        result.push(c);
    }
    result
}

/// Render a path with forward-slash separators for the source-map
/// `sources` entry (the source-map spec is URL-flavored).
fn to_forward_slash(p: &Path) -> String {
    let parts: Vec<String> = p.iter().map(|s| s.to_string_lossy().into_owned()).collect();
    parts.join("/")
}
