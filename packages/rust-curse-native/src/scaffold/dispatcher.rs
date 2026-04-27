//! Render the top-level `lib.rs` / `build.rs` dispatcher files and the
//! per-section `build-{name}.rs` body.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::common::HEADER;

use super::{Activation, BuildContribution, Section};

/// Render the top-level `lib.rs` that inlines the active section's
/// `lib-{name}.rs` via `include!`. Activations must be mutually exclusive
/// at runtime so item names don't collide across sections.
pub(crate) fn render_lib_dispatcher(sections: &BTreeMap<String, Section>) -> String {
    let mut out = String::from(HEADER);

    for (name, section) in sections {
        if section.lib.is_none() {
            continue;
        }
        let cfg_attr = activation_cfg_attr(&section.activation, sections, name);
        if !cfg_attr.is_empty() {
            writeln!(out, "{cfg_attr}").unwrap();
        }
        writeln!(out, "include!(\"lib-{name}.rs\");\n").unwrap();
    }

    out
}

/// Whether a `build.rs` dispatcher is needed: only when at least one
/// section contributes build code or activates via an env var (which
/// requires emitting `cargo:rustc-cfg`).
pub(crate) fn needs_build_rs(sections: &BTreeMap<String, Section>) -> bool {
    sections.values().any(|s| {
        s.build.is_some() || matches!(s.activation, Some(Activation::Env { .. }))
    })
}

/// Render the top-level `build.rs` that includes each section's
/// `build-{name}.rs` (gated by activation) and calls `setup()` on every
/// active section. Env-activated sections also contribute env-var → cfg
/// mappings emitted at the top of `main()`.
pub(crate) fn render_build_dispatcher(sections: &BTreeMap<String, Section>) -> String {
    let mut out = String::from(HEADER);

    for (name, section) in sections {
        if section.build.is_none() {
            continue;
        }
        let cfg_attr = activation_cfg_attr(&section.activation, sections, name);
        if !cfg_attr.is_empty() {
            writeln!(out, "{cfg_attr}").unwrap();
        }
        writeln!(out, "#[path = \"build-{name}.rs\"]").unwrap();
        writeln!(out, "mod build_{name};\n").unwrap();
    }

    writeln!(out, "fn main() {{").unwrap();

    for section in sections.values() {
        if let Some(Activation::Env { env_var, cfg_flag }) = &section.activation {
            writeln!(out, "    if std::env::var(\"{env_var}\").is_ok() {{").unwrap();
            writeln!(out, "        println!(\"cargo:rustc-cfg={cfg_flag}\");").unwrap();
            writeln!(out, "    }}").unwrap();
        }
    }

    for (name, section) in sections {
        if section.build.is_none() {
            continue;
        }
        let cfg_attr = activation_cfg_attr(&section.activation, sections, name);
        if !cfg_attr.is_empty() {
            writeln!(out, "    {cfg_attr}").unwrap();
        }
        writeln!(out, "    build_{name}::setup();").unwrap();
    }

    writeln!(out, "}}").unwrap();
    out
}

/// Render a section's `build-{name}.rs` body: the preamble at the
/// module root, then a `pub fn setup()` containing the main body. Lines
/// inside `main` are auto-indented by 4 spaces.
pub(crate) fn render_section_build(contrib: &BuildContribution) -> String {
    let mut out = String::from(HEADER);
    if !contrib.preamble.is_empty() {
        out.push_str(&contrib.preamble);
        if !contrib.preamble.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }
    writeln!(out, "pub fn setup() {{").unwrap();
    for line in contrib.main.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            writeln!(out, "    {line}").unwrap();
        }
    }
    writeln!(out, "}}").unwrap();
    out
}

fn activation_cfg_attr(
    activation: &Option<Activation>,
    all: &BTreeMap<String, Section>,
    this_name: &str,
) -> String {
    match activation {
        Some(Activation::CargoFeature(f)) => format!("#[cfg(feature = \"{f}\")]"),
        Some(Activation::Env { cfg_flag, .. }) => format!("#[cfg({cfg_flag})]"),
        None => {
            let mut excluded: Vec<String> = Vec::new();
            for (n, s) in all {
                if n == this_name {
                    continue;
                }
                match &s.activation {
                    Some(Activation::CargoFeature(f)) => {
                        excluded.push(format!("feature = \"{f}\""));
                    }
                    Some(Activation::Env { cfg_flag, .. }) => {
                        excluded.push(cfg_flag.clone());
                    }
                    None => {}
                }
            }
            if excluded.is_empty() {
                String::new()
            } else if excluded.len() == 1 {
                format!("#[cfg(not({}))]", excluded[0])
            } else {
                format!("#[cfg(not(any({})))]", excluded.join(", "))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cargo_section(lib: Option<&str>, feature: &str) -> Section {
        Section {
            activation: Some(Activation::CargoFeature(feature.into())),
            lib: lib.map(String::from),
            ..Section::default()
        }
    }

    fn env_section(lib: Option<&str>, env: &str, cfg: &str) -> Section {
        Section {
            activation: Some(Activation::Env {
                env_var: env.into(),
                cfg_flag: cfg.into(),
            }),
            lib: lib.map(String::from),
            ..Section::default()
        }
    }

    fn default_section(lib: Option<&str>) -> Section {
        Section {
            activation: None,
            lib: lib.map(String::from),
            ..Section::default()
        }
    }

    #[test]
    fn lib_dispatcher_default_only() {
        let mut sections = BTreeMap::new();
        sections.insert("default".to_string(), default_section(Some("// d")));

        let out = render_lib_dispatcher(&sections);
        assert!(out.contains("include!(\"lib-default.rs\");"));
        assert!(!out.contains("mod target_"));
        assert!(!out.contains("#[cfg("));
    }

    #[test]
    fn lib_dispatcher_default_excludes_cargo_and_env() {
        let mut sections = BTreeMap::new();
        sections.insert("default".to_string(), default_section(Some("// d")));
        sections.insert("napi".to_string(), cargo_section(Some("// n"), "node"));
        sections.insert(
            "rustdoc".to_string(),
            env_section(Some("// r"), "RUST_CURSE_RUSTDOC", "rust_curse_rustdoc"),
        );

        let out = render_lib_dispatcher(&sections);
        assert!(out.contains("#[cfg(not(any(feature = \"node\", rust_curse_rustdoc)))]"));
        assert!(out.contains("#[cfg(feature = \"node\")]"));
        assert!(out.contains("#[cfg(rust_curse_rustdoc)]"));
        assert!(out.contains("include!(\"lib-default.rs\");"));
        assert!(out.contains("include!(\"lib-napi.rs\");"));
        assert!(out.contains("include!(\"lib-rustdoc.rs\");"));
    }

    #[test]
    fn lib_dispatcher_skips_section_without_lib() {
        let mut sections = BTreeMap::new();
        sections.insert("napi".to_string(), cargo_section(None, "node"));
        let out = render_lib_dispatcher(&sections);
        assert!(!out.contains("include!(\"lib-napi.rs\")"));
    }

    #[test]
    fn needs_build_rs_false_when_only_lib_only_sections() {
        let mut sections = BTreeMap::new();
        sections.insert("default".to_string(), default_section(Some("// d")));
        sections.insert("napi".to_string(), cargo_section(Some("// n"), "node"));
        assert!(!needs_build_rs(&sections));
    }

    #[test]
    fn needs_build_rs_true_with_env_activation() {
        let mut sections = BTreeMap::new();
        sections.insert(
            "rustdoc".to_string(),
            env_section(Some("// r"), "RUST_CURSE_RUSTDOC", "rust_curse_rustdoc"),
        );
        assert!(needs_build_rs(&sections));
    }

    #[test]
    fn needs_build_rs_true_with_build_contribution() {
        let mut sections = BTreeMap::new();
        let napi = Section {
            activation: Some(Activation::CargoFeature("node".into())),
            lib: Some("// n".into()),
            build: Some(BuildContribution {
                preamble: "use napi_build;".into(),
                main: "napi_build::setup();".into(),
            }),
            ..Section::default()
        };
        sections.insert("napi".into(), napi);
        assert!(needs_build_rs(&sections));
    }

    #[test]
    fn build_dispatcher_emits_env_cfg_setup() {
        let mut sections = BTreeMap::new();
        sections.insert(
            "rustdoc".to_string(),
            env_section(Some("// r"), "RUST_CURSE_RUSTDOC", "rust_curse_rustdoc"),
        );
        let out = render_build_dispatcher(&sections);
        assert!(out.contains("if std::env::var(\"RUST_CURSE_RUSTDOC\").is_ok()"));
        assert!(out.contains("cargo:rustc-cfg=rust_curse_rustdoc"));
    }

    #[test]
    fn build_dispatcher_includes_setup_call_for_section_with_build() {
        let mut sections = BTreeMap::new();
        let napi = Section {
            activation: Some(Activation::CargoFeature("node".into())),
            lib: Some("// n".into()),
            build: Some(BuildContribution {
                preamble: "use napi_build;".into(),
                main: "napi_build::setup();".into(),
            }),
            ..Section::default()
        };
        sections.insert("napi".into(), napi);
        let out = render_build_dispatcher(&sections);
        assert!(out.contains("#[cfg(feature = \"node\")]"));
        assert!(out.contains("#[path = \"build-napi.rs\"]"));
        assert!(out.contains("mod build_napi;"));
        assert!(out.contains("build_napi::setup();"));
    }

    #[test]
    fn section_build_renders_preamble_and_setup() {
        let contrib = BuildContribution {
            preamble: "use napi_build;".into(),
            main: "napi_build::setup();".into(),
        };
        let out = render_section_build(&contrib);
        assert!(out.contains("use napi_build;"));
        assert!(out.contains("pub fn setup() {"));
        assert!(out.contains("    napi_build::setup();"));
    }
}
