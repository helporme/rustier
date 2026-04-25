use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use thiserror::Error;

use crate::model::OrphanModule;
use crate::scaffold::rustdoc::RUSTDOC_ENV_VAR;

#[derive(Debug, Error)]
pub enum RustdocError {
    #[error("cargo is not available")]
    CargoNotAvailable(#[source] io::Error),

    #[error("cargo metadata failed (exit code {exit_code:?}):\n{stderr}")]
    MetadataFailed {
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("failed to parse cargo metadata output")]
    MetadataParseFailed(#[source] serde_json::Error),

    #[error("no library target found in cargo metadata")]
    NoLibTarget,

    #[error("cargo rustdoc failed (exit code {exit_code:?}):\n{stderr}")]
    RustdocFailed {
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("rustdoc JSON not found at '{}'", .0.display())]
    JsonNotFound(PathBuf),

    #[error("failed to read rustdoc JSON from '{}'", .0.display())]
    JsonReadFailed(PathBuf, #[source] io::Error),

    #[error("failed to parse rustdoc JSON")]
    JsonParseFailed(#[source] serde_json::Error),
}

/// Generates rustdoc in json format.
///
/// Requires nightly version of rust installed.
pub fn generate_rustdoc(
    project_root_abs_path: &Path,
) -> Result<rustdoc_types::Crate, RustdocError> {
    let meta = cargo_metadata(project_root_abs_path)?;
    run_cargo_rustdoc_json(
        project_root_abs_path,
        Path::new(&meta.target_dir),
        &meta.lib_name,
    )
}

/// Runs `cargo +nightly rustdoc --output-format json` with a caller-supplied
/// target directory and lib name, skipping the `cargo metadata` lookup that
/// `generate_rustdoc` performs. Used by tests that build synthetic temp
/// crates where `cargo metadata --frozen` would fail for lack of Cargo.lock.
pub fn run_cargo_rustdoc_json(
    project_abs_path: &Path,
    target_abs_dir: &Path,
    lib_name: &str,
) -> Result<rustdoc_types::Crate, RustdocError> {
    let output = Command::new("cargo")
        .args([
            "+nightly",
            "rustdoc",
            "--lib",
            "--",
            "-Z",
            "unstable-options",
            "--output-format",
            "json",
        ])
        .env(RUSTDOC_ENV_VAR, "1")
        .env("CARGO_TARGET_DIR", target_abs_dir)
        .current_dir(project_abs_path)
        .output()
        .map_err(RustdocError::CargoNotAvailable)?;

    if !output.status.success() {
        return Err(RustdocError::RustdocFailed {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let json_abs_path = target_abs_dir.join("doc").join(format!("{lib_name}.json"));

    if !json_abs_path.exists() {
        return Err(RustdocError::JsonNotFound(json_abs_path));
    }

    let json = fs::read_to_string(&json_abs_path)
        .map_err(|e| RustdocError::JsonReadFailed(json_abs_path, e))?;

    serde_json::from_str(&json).map_err(RustdocError::JsonParseFailed)
}

struct CrateMetadata {
    /// Absolute path to the target directory (shared in workspaces).
    target_dir: String,
    /// Library target name (hyphens already replaced with underscores).
    lib_name: String,
}

fn cargo_metadata(abs_cargo_dir: &Path) -> Result<CrateMetadata, RustdocError> {
    let output = Command::new("cargo")
        .args(["metadata", "--frozen", "--format-version", "1", "--no-deps"])
        .current_dir(abs_cargo_dir)
        .output()
        .map_err(RustdocError::CargoNotAvailable)?;

    if !output.status.success() {
        return Err(RustdocError::MetadataFailed {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(RustdocError::MetadataParseFailed)?;

    let target_dir = json["target_directory"]
        .as_str()
        .ok_or(RustdocError::NoLibTarget)?
        .to_owned();

    // Find the library target name from the first package's targets.
    let lib_name = json["packages"]
        .as_array()
        .and_then(|pkgs| pkgs.first())
        .and_then(|pkg| pkg["targets"].as_array())
        .and_then(|targets| {
            targets.iter().find(|t| {
                t["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|k| k.as_str() == Some("lib")))
            })
        })
        .and_then(|t| t["name"].as_str())
        .ok_or(RustdocError::NoLibTarget)?
        .to_owned();

    Ok(CrateMetadata {
        target_dir,
        lib_name,
    })
}

/// Match rustdoc items to orphan modules by comparing `item.span.filename`
/// against each module's source path.
pub fn link_rustdoc_items(
    crate_: &rustdoc_types::Crate,
    modules: &mut [OrphanModule],
    module_dir_rel_path: &Path,
) {
    // Build a lookup: normalized span filename -> module index.
    // Rustdoc span filenames may contain `..` segments (e.g. `.rust-curse/../src/greet.rs`),
    // so we normalize both sides before comparing.
    let filename_to_idx: HashMap<_, _> = modules
        .iter()
        .enumerate()
        .map(|(idx, m)| {
            let expected = normalize_path(&module_dir_rel_path.join(&m.rel_path));
            (expected, idx)
        })
        .collect();

    for item in crate_.index.values() {
        let Some(span) = &item.span else {
            continue;
        };

        let normalized = normalize_path(&span.filename);
        if let Some(&idx) = filename_to_idx.get(&normalized) {
            modules[idx].item_ids.push(item.id);
        }
    }
}

/// Normalize a relative path by resolving `.` and `..` components logically
/// (without touching the filesystem).
fn normalize_path(path: &Path) -> PathBuf {
    let mut parts: Vec<Component> = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                if let Some(Component::Normal(_)) = parts.last() {
                    parts.pop();
                } else {
                    parts.push(component);
                }
            }
            Component::CurDir => {}
            _ => parts.push(component),
        }
    }
    parts.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustdoc_types::{Crate, Id, Item, ItemEnum, Span};

    fn make_item(id_num: u32, name: &str, filename: &str) -> (Id, Item) {
        let id = Id(id_num);
        let item = Item {
            id,
            crate_id: 0,
            name: Some(name.to_string()),
            span: Some(Span {
                filename: PathBuf::from(filename),
                begin: (1, 0),
                end: (1, 0),
            }),
            visibility: rustdoc_types::Visibility::Public,
            docs: None,
            links: HashMap::new(),
            attrs: Vec::new(),
            deprecation: None,
            inner: ItemEnum::Function(rustdoc_types::Function {
                sig: rustdoc_types::FunctionSignature {
                    inputs: Vec::new(),
                    output: None,
                    is_c_variadic: false,
                },
                generics: rustdoc_types::Generics {
                    params: Vec::new(),
                    where_predicates: Vec::new(),
                },
                header: rustdoc_types::FunctionHeader {
                    is_const: false,
                    is_unsafe: false,
                    is_async: false,
                    abi: rustdoc_types::Abi::Rust,
                },
                has_body: true,
            }),
        };
        (id, item)
    }

    fn make_crate(items: Vec<(Id, Item)>) -> Crate {
        let mut index = HashMap::new();
        for (id, item) in items {
            index.insert(id, item);
        }
        Crate {
            root: Id(0),
            crate_version: None,
            includes_private: false,
            index,
            paths: HashMap::new(),
            external_crates: HashMap::new(),
            target: rustdoc_types::Target {
                triple: String::new(),
                target_features: Vec::new(),
            },
            format_version: 0,
        }
    }

    #[test]
    fn links_items_to_modules() {
        let crate_ = make_crate(vec![
            make_item(1, "hello", "src/greet.rs"),
            make_item(2, "add", "src/math.rs"),
            make_item(3, "Point", "src/math.rs"),
        ]);

        let mut modules = vec![
            OrphanModule {
                name: "greet".into(),
                rel_path: PathBuf::from("greet.rs"),
                item_ids: Vec::new(),
            },
            OrphanModule {
                name: "math".into(),
                rel_path: PathBuf::from("math.rs"),
                item_ids: Vec::new(),
            },
        ];

        link_rustdoc_items(&crate_, &mut modules, Path::new("src"));

        assert_eq!(modules[0].item_ids.len(), 1, "greet should have 1 item");
        assert_eq!(modules[1].item_ids.len(), 2, "math should have 2 items");
    }

    #[test]
    fn ignores_items_without_span() {
        let (id, mut item) = make_item(1, "phantom", "src/greet.rs");
        item.span = None;
        let crate_ = make_crate(vec![(id, item)]);

        let mut modules = vec![OrphanModule {
            name: "greet".into(),
            rel_path: PathBuf::from("greet.rs"),
            item_ids: Vec::new(),
        }];

        link_rustdoc_items(&crate_, &mut modules, Path::new("src"));

        assert!(modules[0].item_ids.is_empty());
    }

    #[test]
    fn ignores_items_from_other_files() {
        let crate_ = make_crate(vec![
            make_item(1, "hello", "src/greet.rs"),
            make_item(2, "external", "src/other.rs"),
        ]);

        let mut modules = vec![OrphanModule {
            name: "greet".into(),
            rel_path: PathBuf::from("greet.rs"),
            item_ids: Vec::new(),
        }];

        link_rustdoc_items(&crate_, &mut modules, Path::new("src"));

        assert_eq!(modules[0].item_ids.len(), 1);
    }

    #[test]
    fn nested_module_paths() {
        let crate_ = make_crate(vec![make_item(1, "go", "src/nested/deep.rs")]);

        let mut modules = vec![OrphanModule {
            name: "nested::deep".into(),
            rel_path: PathBuf::from("nested/deep.rs"),
            item_ids: Vec::new(),
        }];

        link_rustdoc_items(&crate_, &mut modules, Path::new("src"));

        assert_eq!(modules[0].item_ids.len(), 1);
    }

    #[test]
    fn normalizes_dotdot_in_span_filenames() {
        // Rustdoc emits paths like `.rust-curse/../src/greet.rs` for #[path] modules
        let crate_ = make_crate(vec![make_item(1, "hello", ".rust-curse/../src/greet.rs")]);

        let mut modules = vec![OrphanModule {
            name: "greet".into(),
            rel_path: PathBuf::from("greet.rs"),
            item_ids: Vec::new(),
        }];

        link_rustdoc_items(&crate_, &mut modules, Path::new("src"));

        assert_eq!(modules[0].item_ids.len(), 1);
    }

    #[test]
    fn normalize_path_resolves_dotdot() {
        assert_eq!(normalize_path(Path::new("a/b/../c")), PathBuf::from("a/c"));
        assert_eq!(
            normalize_path(Path::new(".rust-curse/../src/greet.rs")),
            PathBuf::from("src/greet.rs"),
        );
        assert_eq!(
            normalize_path(Path::new("src/greet.rs")),
            PathBuf::from("src/greet.rs")
        );
        assert_eq!(
            normalize_path(Path::new("./src/greet.rs")),
            PathBuf::from("src/greet.rs")
        );
    }
}
