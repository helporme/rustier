//! Step 1b.
//! Discovers public types from crate dependencies using `cargo metadata`.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use crate::steps::s1_discovery::{self, RustModule};

/// Discovers public types from direct dependencies of the crate at `cargo_path`.
/// Returns a map of crate_name -> root RustModule (with nested members).
pub fn discover_dependencies(cargo_path: &str) -> HashMap<String, RustModule> {
    let cargo_dir = Path::new(cargo_path)
        .parent()
        .expect("Cargo.toml must have a parent directory");

    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .current_dir(cargo_dir)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return HashMap::new(),
    };

    let json: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    // Find the root package to get its direct dependencies
    let resolve = match json.get("resolve") {
        Some(r) => r,
        None => return HashMap::new(),
    };

    let root_id = match resolve.get("root").and_then(|r| r.as_str()) {
        Some(id) => id,
        None => return HashMap::new(),
    };

    // Get direct dependency package IDs from the resolve graph
    let direct_dep_ids: Vec<&str> = resolve
        .get("nodes")
        .and_then(|n| n.as_array())
        .into_iter()
        .flatten()
        .find(|node| node.get("id").and_then(|id| id.as_str()) == Some(root_id))
        .and_then(|node| node.get("deps"))
        .and_then(|deps| deps.as_array())
        .into_iter()
        .flatten()
        .filter_map(|dep| dep.get("pkg").and_then(|p| p.as_str()))
        .collect();

    let packages = match json.get("packages").and_then(|p| p.as_array()) {
        Some(pkgs) => pkgs,
        None => return HashMap::new(),
    };

    let mut result = HashMap::new();

    for pkg in packages {
        let pkg_id = match pkg.get("id").and_then(|id| id.as_str()) {
            Some(id) => id,
            None => continue,
        };

        if !direct_dep_ids.contains(&pkg_id) {
            continue;
        }

        let crate_name = match pkg.get("name").and_then(|n| n.as_str()) {
            Some(name) => name.replace('-', "_"),
            None => continue,
        };

        let manifest_path = match pkg.get("manifest_path").and_then(|p| p.as_str()) {
            Some(p) => p,
            None => continue,
        };

        let crate_dir = match Path::new(manifest_path).parent() {
            Some(d) => d,
            None => continue,
        };

        let lib_rs = crate_dir.join("src").join("lib.rs");
        if let Some(module) = s1_discovery::discover_crate(&lib_rs, &crate_name) {
            result.insert(crate_name, module);
        }
    }

    result
}
