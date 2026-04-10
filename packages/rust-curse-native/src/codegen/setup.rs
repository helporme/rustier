//! Generate `lib.rs` and `build.rs` for the output directory.

use crate::codegen::common::HEADER;
use crate::model::GeneratedFile;
use crate::session::{BuildTarget, RUSTDOC_CFG, RUSTDOC_ENV_VAR, RUSTDOC_FILE};
use std::fmt::Write;
use std::path::PathBuf;

/// Generate a feature-gated `lib.rs` that delegates to the active target module.
pub fn generate_lib_rs(targets: &[BuildTarget]) -> GeneratedFile {
    let has_default = targets
        .iter()
        .any(|t| matches!(t, BuildTarget::Default { .. }));

    let mut content = String::from(HEADER);

    // Default fallback (no features, no rustdoc env)
    if has_default {
        let mut non_default: Vec<String> = vec![RUSTDOC_CFG.to_string()];
        for t in targets {
            if let Some(feature) = t.feature_name() {
                non_default.push(format!("feature = \"{feature}\""));
            }
        }

        let not_condition = if non_default.len() == 1 {
            format!("#[cfg(not({}))]", non_default[0])
        } else {
            format!("#[cfg(not(any({})))]", non_default.join(", "))
        };

        writeln!(content, "{not_condition}").unwrap();
        writeln!(content, "#[path = \"default.rs\"]").unwrap();
        writeln!(content, "mod target;\n").unwrap();
    }

    // Rustdoc target (always present, activated via build.rs cfg)
    writeln!(content, "#[cfg({RUSTDOC_CFG})]").unwrap();
    writeln!(content, "#[path = \"{RUSTDOC_FILE}\"]").unwrap();
    writeln!(content, "mod target;\n").unwrap();

    // Feature-gated targets (node, wasm)
    for target in targets {
        let Some(feature) = target.feature_name() else {
            continue;
        };
        writeln!(content, "#[cfg(feature = \"{feature}\")]").unwrap();
        writeln!(content, "#[path = \"{}\"]", target.file_name()).unwrap();
        writeln!(content, "mod target;\n").unwrap();
    }

    writeln!(content, "pub use target::*;").unwrap();

    GeneratedFile {
        rel_path: PathBuf::from("lib.rs"),
        content,
    }
}

/// Generate a `build.rs` that sets the `rust_curse_rustdoc` cfg flag
/// when the `RUST_CURSE_RUSTDOC` env var is present, and includes
/// `napi_build::setup()` when a Node target is configured.
pub fn generate_build_rs(targets: &[BuildTarget]) -> GeneratedFile {
    let has_node = targets
        .iter()
        .any(|t| matches!(t, BuildTarget::Node { .. }));

    let mut content = String::new();

    if has_node {
        writeln!(content, "extern crate napi_build;\n").unwrap();
    }

    writeln!(content, "fn main() {{").unwrap();
    writeln!(
        content,
        "    if std::env::var(\"{RUSTDOC_ENV_VAR}\").is_ok() {{"
    )
    .unwrap();
    writeln!(
        content,
        "        println!(\"cargo:rustc-cfg={RUSTDOC_CFG}\");"
    )
    .unwrap();
    writeln!(content, "    }}").unwrap();

    if has_node {
        writeln!(content, "    napi_build::setup();").unwrap();
    }

    writeln!(content, "}}").unwrap();

    GeneratedFile {
        rel_path: PathBuf::from("build.rs"),
        content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SourceOptions;

    fn default_target() -> BuildTarget {
        BuildTarget::Default {
            js_source: SourceOptions {
                include: vec![],
                exclude: vec![],
            },
        }
    }

    fn node_target() -> BuildTarget {
        BuildTarget::Node {
            js_source: SourceOptions {
                include: vec![],
                exclude: vec![],
            },
            feature_name: "target-node".into(),
        }
    }

    fn wasm_target() -> BuildTarget {
        BuildTarget::Wasm {
            js_source: SourceOptions {
                include: vec![],
                exclude: vec![],
            },
            feature_name: "target-wasm".into(),
        }
    }

    #[test]
    fn default_only_has_rustdoc_and_default() {
        let file = generate_lib_rs(&[default_target()]);
        assert!(file.content.contains("#[path = \"default.rs\"]"));
        assert!(file.content.contains("#[path = \"rustdoc.rs\"]"));
        assert!(!file.content.contains("#[path = \"node.rs\"]"));
        assert!(!file.content.contains("#[path = \"wasm.rs\"]"));
        assert!(file
            .content
            .contains(&format!("#[cfg(not({RUSTDOC_CFG}))]")));
    }

    #[test]
    fn node_target_includes_node() {
        let file = generate_lib_rs(&[default_target(), node_target()]);
        assert!(file.content.contains("#[path = \"default.rs\"]"));
        assert!(file.content.contains("#[path = \"rustdoc.rs\"]"));
        assert!(file.content.contains("#[path = \"node.rs\"]"));
        assert!(!file.content.contains("#[path = \"wasm.rs\"]"));
        assert!(file.content.contains("feature = \"target-node\""));
    }

    #[test]
    fn node_only_no_default_fallback() {
        let file = generate_lib_rs(&[node_target()]);
        assert!(!file.content.contains("#[path = \"default.rs\"]"));
        assert!(file.content.contains("#[path = \"rustdoc.rs\"]"));
        assert!(file.content.contains("#[path = \"node.rs\"]"));
    }

    #[test]
    fn custom_feature_names() {
        let file = generate_lib_rs(&[BuildTarget::Node {
            js_source: SourceOptions {
                include: vec![],
                exclude: vec![],
            },
            feature_name: "_my-node".into(),
        }]);
        assert!(file.content.contains("feature = \"_my-node\""));
        assert!(!file.content.contains("target-node"));
    }

    #[test]
    fn all_targets() {
        let file = generate_lib_rs(&[default_target(), node_target(), wasm_target()]);
        assert!(file.content.contains("#[path = \"default.rs\"]"));
        assert!(file.content.contains("#[path = \"rustdoc.rs\"]"));
        assert!(file.content.contains("#[path = \"node.rs\"]"));
        assert!(file.content.contains("#[path = \"wasm.rs\"]"));
        assert!(file.content.contains("pub use target::*;"));
    }

    #[test]
    fn build_rs_has_env_check() {
        let file = generate_build_rs(&[default_target()]);
        assert_eq!(file.rel_path, PathBuf::from("build.rs"));
        assert!(file.content.contains(RUSTDOC_ENV_VAR));
        assert!(file.content.contains(RUSTDOC_CFG));
        assert!(!file.content.contains("napi_build"));
    }

    #[test]
    fn build_rs_includes_napi_setup_for_node() {
        let file = generate_build_rs(&[node_target()]);
        assert!(file.content.contains("extern crate napi_build;"));
        assert!(file.content.contains("napi_build::setup();"));
        assert!(file.content.contains(RUSTDOC_ENV_VAR));
    }
}
