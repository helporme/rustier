//! Render the top-level `lib.rs` / `build.rs` dispatcher files and the
//! per-section `build-{name}.rs` body.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::common::HEADER;

use super::{Activation, BuildContribution, Section};

/// Render the top-level `lib.rs` that inlines each section's
/// `lib-{name}.rs` via `include!`. Sections without an activation are
/// always linked in; cargo-feature sections are gated by their feature.
/// At most one `ExclusiveEnv` section may exist; when its env var is
/// set, every other section is suppressed.
pub(crate) fn render_lib_dispatcher(sections: &BTreeMap<String, Section>) -> String {
    let mut out = String::from(HEADER);
    out.push_str("#![allow(non_camel_case_types, non_snake_case)]\n\n");

    let exclusive_cfg = exclusive_cfg_flag(sections);

    for (name, section) in sections {
        if section.content.is_none() {
            continue;
        }
        let cfg_attr = activation_cfg_attr(&section.activation, exclusive_cfg.as_deref());
        if !cfg_attr.is_empty() {
            writeln!(out, "{cfg_attr}").unwrap();
        }
        writeln!(out, "include!(\"lib-{name}.rs\");\n").unwrap();
    }

    out
}

/// Whether a `build.rs` dispatcher is needed: only when at least one
/// section contributes build code or activates via an exclusive env var
/// (which requires emitting `cargo:rustc-cfg`).
pub(crate) fn needs_build_rs(sections: &BTreeMap<String, Section>) -> bool {
    sections.values().any(|s| {
        s.build_content.is_some() || matches!(s.activation, Some(Activation::ExclusiveEnv { .. }))
    })
}

/// Render the top-level `build.rs` that includes each section's
/// `build-{name}.rs` (gated by activation) and calls `setup()` on every
/// active section. Exclusive-env-activated sections also contribute
/// env-var → cfg mappings emitted at the top of `main()`.
pub(crate) fn render_build_dispatcher(sections: &BTreeMap<String, Section>) -> String {
    let mut out = String::from(HEADER);

    let exclusive_cfg = exclusive_cfg_flag(sections);

    for (name, section) in sections {
        if section.build_content.is_none() {
            continue;
        }
        let cfg_attr = activation_cfg_attr(&section.activation, exclusive_cfg.as_deref());
        if !cfg_attr.is_empty() {
            writeln!(out, "{cfg_attr}").unwrap();
        }
        writeln!(out, "#[path = \"build-{name}.rs\"]").unwrap();
        writeln!(out, "mod build_{name};\n").unwrap();
    }

    writeln!(out, "fn main() {{").unwrap();

    for section in sections.values() {
        if let Some(Activation::ExclusiveEnv { env_var, cfg_flag }) = &section.activation {
            writeln!(out, "    if std::env::var(\"{env_var}\").is_ok() {{").unwrap();
            writeln!(out, "        println!(\"cargo:rustc-cfg={cfg_flag}\");").unwrap();
            writeln!(out, "    }}").unwrap();
        }
    }

    for (name, section) in sections {
        if section.build_content.is_none() {
            continue;
        }
        let cfg_attr = activation_cfg_attr(&section.activation, exclusive_cfg.as_deref());
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

fn exclusive_cfg_flag(sections: &BTreeMap<String, Section>) -> Option<String> {
    sections.values().find_map(|s| match &s.activation {
        Some(Activation::ExclusiveEnv { cfg_flag, .. }) => Some(cfg_flag.clone()),
        _ => None,
    })
}

fn activation_cfg_attr(activation: &Option<Activation>, exclusive_cfg: Option<&str>) -> String {
    match activation {
        Some(Activation::CargoFeature(f)) => match exclusive_cfg {
            Some(ex) => format!("#[cfg(all(feature = \"{f}\", not({ex})))]"),
            None => format!("#[cfg(feature = \"{f}\")]"),
        },
        Some(Activation::ExclusiveEnv { cfg_flag, .. }) => format!("#[cfg({cfg_flag})]"),
        None => match exclusive_cfg {
            Some(ex) => format!("#[cfg(not({ex}))]"),
            None => String::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cargo_section(content: Option<&str>, feature: &str) -> Section {
        Section {
            activation: Some(Activation::CargoFeature(feature.into())),
            content: content.map(String::from),
            ..Section::default()
        }
    }

    fn exclusive_env_section(content: Option<&str>, env: &str, cfg: &str) -> Section {
        Section {
            activation: Some(Activation::ExclusiveEnv {
                env_var: env.into(),
                cfg_flag: cfg.into(),
            }),
            content: content.map(String::from),
            ..Section::default()
        }
    }

    fn always_section(content: Option<&str>) -> Section {
        Section {
            activation: None,
            content: content.map(String::from),
            ..Section::default()
        }
    }

    #[test]
    fn lib_dispatcher_always_only() {
        let mut sections = BTreeMap::new();
        sections.insert("link".to_string(), always_section(Some("// l")));

        let out = render_lib_dispatcher(&sections);
        assert!(out.contains("#![allow(non_camel_case_types, non_snake_case)]"));
        assert!(out.contains("include!(\"lib-link.rs\");"));
        assert!(!out.contains("#[cfg("));
    }

    #[test]
    fn lib_dispatcher_always_and_cargo_coexist_without_exclusive() {
        let mut sections = BTreeMap::new();
        sections.insert("link".to_string(), always_section(Some("// l")));
        sections.insert("napi".to_string(), cargo_section(Some("// n"), "node"));

        let out = render_lib_dispatcher(&sections);
        assert!(out.contains("include!(\"lib-link.rs\");"));
        assert!(out.contains("#[cfg(feature = \"node\")]"));
        assert!(out.contains("include!(\"lib-napi.rs\");"));
        // No exclusive env in play; "always" section gets no cfg attr.
        assert!(
            !out.contains("#[cfg(not("),
            "no exclusive env exists, so no `cfg(not(...))` should be emitted; got:\n{out}"
        );
    }

    #[test]
    fn lib_dispatcher_exclusive_suppresses_others() {
        let mut sections = BTreeMap::new();
        sections.insert("link".to_string(), always_section(Some("// l")));
        sections.insert("napi".to_string(), cargo_section(Some("// n"), "node"));
        sections.insert(
            "rustdoc".to_string(),
            exclusive_env_section(Some("// r"), "RUST_CURSE_RUSTDOC", "rust_curse_rustdoc"),
        );

        let out = render_lib_dispatcher(&sections);
        assert!(out.contains("#[cfg(not(rust_curse_rustdoc))]"));
        assert!(out.contains("#[cfg(all(feature = \"node\", not(rust_curse_rustdoc)))]"));
        assert!(out.contains("#[cfg(rust_curse_rustdoc)]"));
        assert!(out.contains("include!(\"lib-link.rs\");"));
        assert!(out.contains("include!(\"lib-napi.rs\");"));
        assert!(out.contains("include!(\"lib-rustdoc.rs\");"));
    }

    #[test]
    fn lib_dispatcher_skips_section_without_content() {
        let mut sections = BTreeMap::new();
        sections.insert("napi".to_string(), cargo_section(None, "node"));
        let out = render_lib_dispatcher(&sections);
        assert!(!out.contains("include!(\"lib-napi.rs\")"));
    }

    #[test]
    fn needs_build_rs_false_when_only_content_only_sections() {
        let mut sections = BTreeMap::new();
        sections.insert("link".to_string(), always_section(Some("// l")));
        sections.insert("napi".to_string(), cargo_section(Some("// n"), "node"));
        assert!(!needs_build_rs(&sections));
    }

    #[test]
    fn needs_build_rs_true_with_exclusive_env_activation() {
        let mut sections = BTreeMap::new();
        sections.insert(
            "rustdoc".to_string(),
            exclusive_env_section(Some("// r"), "RUST_CURSE_RUSTDOC", "rust_curse_rustdoc"),
        );
        assert!(needs_build_rs(&sections));
    }

    #[test]
    fn needs_build_rs_true_with_build_contribution() {
        let mut sections = BTreeMap::new();
        let napi = Section {
            activation: Some(Activation::CargoFeature("node".into())),
            content: Some("// n".into()),
            build_content: Some(BuildContribution {
                preamble: "use napi_build;".into(),
                main: "napi_build::setup();".into(),
            }),
            ..Section::default()
        };
        sections.insert("napi".into(), napi);
        assert!(needs_build_rs(&sections));
    }

    #[test]
    fn build_dispatcher_emits_exclusive_env_cfg_setup() {
        let mut sections = BTreeMap::new();
        sections.insert(
            "rustdoc".to_string(),
            exclusive_env_section(Some("// r"), "RUST_CURSE_RUSTDOC", "rust_curse_rustdoc"),
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
            content: Some("// n".into()),
            build_content: Some(BuildContribution {
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
