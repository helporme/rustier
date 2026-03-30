//! Step 6 for Wasm target.
//! Generates per-module JS glue files that wrap wasm-bindgen bindings,
//! re-exporting functions with friendly names.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use crate::steps::s1_discovery::{RustModuleFn, RustModuleMember};
use crate::steps::s3_treeshake::RustModuleShaked;

struct FnGlue {
    wasm_name: String,
    export_name: String,
    arg_names: Vec<String>,
}

/// Generates JS glue modules only for shaked (imported) modules.
pub fn generate_js_glue(
    shaked: &[RustModuleShaked],
    binding_file: &str,
    output_dir: &Path,
    project_root: &Path,
) {
    let root = fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    fs::create_dir_all(output_dir).expect("failed to create output directory");

    let abs_output_dir = fs::canonicalize(output_dir).unwrap_or_else(|_| output_dir.to_path_buf());
    let abs_binding_file = abs_output_dir
        .join(binding_file)
        .to_string_lossy()
        .into_owned();

    let mut index: HashMap<String, String> = HashMap::new();

    for s in shaked {
        let module = s.module;
        let members = match &module.members {
            Some(m) => m,
            None => continue,
        };

        let module_path = Path::new(&module.path);
        let relative = module_path.strip_prefix(&root).unwrap_or(module_path);

        let source_key = if relative.file_name().is_some_and(|n| n == "mod.rs") {
            format!("./{}", relative.parent().unwrap_or(relative).display())
        } else {
            format!("./{}", relative.display())
        };

        let glue_name = source_key
            .trim_start_matches("./")
            .replace('/', "_")
            .replace(".rs", "");
        let suffix = short_hash(&source_key);
        let glue_filename = format!("{}_{}.js", glue_name, suffix);

        let imported: HashSet<&str> = s.imported_members.iter().map(|s| s.as_str()).collect();
        let content = generate_module_glue(&abs_binding_file, members, &imported);

        fs::write(output_dir.join(&glue_filename), &content)
            .expect("failed to write glue JS file");

        index.insert(source_key, glue_filename);
    }

    let json = serde_json::to_string_pretty(&index).expect("failed to serialize modules.json");
    fs::write(output_dir.join("modules.json"), json).expect("failed to write modules.json");
}

fn generate_module_glue(
    binding_file: &str,
    members: &[RustModuleMember],
    imported: &HashSet<&str>,
) -> String {
    let mut fns = Vec::new();
    collect_fns(members, imported, &mut fns);

    let mut out = String::new();

    if !fns.is_empty() {
        let import_list: Vec<String> = fns
            .iter()
            .map(|f| format!("{} as $_{}", f.wasm_name, f.wasm_name))
            .collect();
        writeln!(
            out,
            "import init, {{ {} }} from '{}';",
            import_list.join(", "),
            binding_file
        )
        .unwrap();
        writeln!(out, "await init();").unwrap();
        writeln!(out).unwrap();
    }

    for f in &fns {
        let params = f.arg_names.join(", ");
        writeln!(
            out,
            "export function {}({}) {{ return $_{}({}); }}",
            f.export_name, params, f.wasm_name, params
        )
        .unwrap();
    }

    out
}

fn collect_fns(
    members: &[RustModuleMember],
    imported: &HashSet<&str>,
    fns: &mut Vec<FnGlue>,
) {
    for member in members {
        match member {
            RustModuleMember::Fn(func) => {
                let short_name = func.name.rsplit("::").next().unwrap_or(&func.name);
                if !imported.contains(short_name) {
                    continue;
                }
                fns.push(make_fn_glue(func));
            }
            RustModuleMember::Module(sub) => {
                let short_name = sub.name.rsplit("::").next().unwrap_or(&sub.name);
                if !imported.contains(short_name) {
                    continue;
                }
                if let Some(ref sub_members) = sub.members {
                    collect_all_fns(sub_members, fns);
                }
            }
        }
    }
}

fn collect_all_fns(members: &[RustModuleMember], fns: &mut Vec<FnGlue>) {
    for member in members {
        match member {
            RustModuleMember::Fn(func) => {
                fns.push(make_fn_glue(func));
            }
            RustModuleMember::Module(sub) => {
                if let Some(ref sub_members) = sub.members {
                    collect_all_fns(sub_members, fns);
                }
            }
        }
    }
}

fn make_fn_glue(func: &RustModuleFn) -> FnGlue {
    let short_name = func.name.rsplit("::").next().unwrap_or(&func.name);
    let wasm_name = func.name.replace("::", "__");
    let arg_names = func.args.iter().map(|a| a.name.clone()).collect();
    FnGlue {
        wasm_name,
        export_name: short_name.to_string(),
        arg_names,
    }
}

fn short_hash(input: &str) -> String {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:08x}", hasher.finish() & 0xFFFFFFFF)
}
