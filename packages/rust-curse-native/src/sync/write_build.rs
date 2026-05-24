use std::{collections::HashMap, fmt, io, path::Path};

use super::write::write_if_changed;

/// A representation of a crate build.rs where it's content conditionally
/// compiled in based on the presence of cargo features, hence "switch" in
/// name. Renders to a single `build.rs` file.
///
/// Example:
/// ```no_run
/// use rust_curse::sync::write_build::*;
///
/// let mut switch = BuildSwitch::new();
/// switch
///     .set_branch(
///         "default",
///         BuildBranch::new("println!(\"cargo:rerun-if-changed=src\");"),
///     )
///     .set_branch(
///         "napi",
///         BuildBranch::new("use napi_build;\nnapi_build::setup();")
///             .requires_cargo_feature("node"),
///     );
///
/// let build_rs = render_build(&switch).unwrap();
/// assert_eq!(
///     build_rs,
///     "fn main() {\n\
///      \x20   {\n\
///      \x20       println!(\"cargo:rerun-if-changed=src\");\n\
///      \x20   }\n\
///      \n\
///      \x20   #[cfg(feature = \"node\")]\n\
///      \x20   {\n\
///      \x20       use napi_build;\n\
///      \x20       napi_build::setup();\n\
///      \x20   }\n\
///      }\n",
/// );
/// ```
#[derive(Default, Debug)]
pub struct BuildSwitch {
    pub branches: HashMap<String, BuildBranch>,
}

impl BuildSwitch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a branch that is included whenever its requirements are met,
    /// alongside any other matching branches; replaces any existing branch
    /// with the same name.
    pub fn set_branch(&mut self, name: impl Into<String>, build: BuildBranch) -> &mut Self {
        self.branches.insert(name.into(), build);
        self
    }
}

/// A branch of a [`BuildSwitch`]. Its `content` is inlined verbatim inside a
/// cfg-gated block in the rendered `build.rs`; both `use` statements and
/// setup calls live in the same string.
#[derive(Default, Debug)]
pub struct BuildBranch {
    pub content: String,
    pub req_cargo_features: Option<Vec<String>>,
}

impl BuildBranch {
    /// Creates a new [`BuildBranch`] with the given content.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            ..Default::default()
        }
    }

    /// Requires the given cargo feature to be enabled.
    pub fn requires_cargo_feature(mut self, feature: impl Into<String>) -> Self {
        let mut req_cargo_features = self.req_cargo_features.unwrap_or_default();
        req_cargo_features.push(feature.into());
        self.req_cargo_features = Some(req_cargo_features);
        self
    }
}

impl fmt::Display for BuildSwitch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.branches.is_empty() {
            return writeln!(f, "branches: []");
        }
        let mut sorted: Vec<(&String, &BuildBranch)> = self.branches.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(b.0));
        writeln!(f, "branches:")?;
        for (name, build) in sorted {
            write!(f, "  - {name}")?;
            if let Some(features) = build.req_cargo_features.as_ref().filter(|v| !v.is_empty()) {
                write!(f, " cargo_features={features:?}")?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

/// Returns the body of a build branch that translates an env-var presence
/// check into a `cargo:rustc-cfg` directive. The cfg flag mirrors the env
/// var name in lowercase. Also emits `cargo:rerun-if-env-changed` so cargo
pub fn env_flag_build_content(env_flag: impl Into<String>) -> String {
    let env_flag = env_flag.into();
    let cfg_flag = env_flag.to_lowercase();
    format!(
        "println!(\"cargo:rerun-if-env-changed={env_flag}\");\n\
         println!(\"cargo:rustc-check-cfg=cfg({cfg_flag})\");\n\
         if std::env::var(\"{env_flag}\").is_ok() {{\n    println!(\"cargo:rustc-cfg={cfg_flag}\");\n}}\n"
    )
}

/// Error returned by [`write_build`] when writing to disk fails.
#[derive(Debug, thiserror::Error)]
pub enum WriteBuildError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

/// Writes the rendered [`BuildSwitch`] to `<dir_path>/build.rs`. No-op when
/// [`render_build`] has nothing to emit.
pub fn write_build(dir_path: &Path, switch: &BuildSwitch) -> Result<(), WriteBuildError> {
    if let Some(content) = render_build(switch) {
        write_if_changed(&dir_path.join("build.rs"), content.as_bytes())?;
    }
    Ok(())
}

/// Renders the given [`BuildSwitch`] into a complete `build.rs` file.
///
/// Each branch becomes a cfg-gated `{ ... }` block inside `fn main()` with
/// the branch's content inlined inside. Branches are emitted in alphabetical
/// order. Returns [`None`] when there are no branches (an empty `fn main()`
/// would have nothing to do).
pub fn render_build(switch: &BuildSwitch) -> Option<String> {
    let mut sorted: Vec<(&String, &BuildBranch)> = switch.branches.iter().collect();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    let mut out = String::from("fn main() {\n");
    for (i, (_, build)) in sorted.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if let Some(cfg) = cfg_for_build(build) {
            out.push_str("    ");
            out.push_str(&cfg);
            out.push('\n');
        }
        out.push_str("    {\n");
        push_indented(&mut out, &build.content);
        out.push_str("    }\n");
    }
    out.push_str("}\n");
    Some(out)
}

fn push_indented(out: &mut String, code: &str) {
    for line in code.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            out.push_str("        ");
            out.push_str(line);
            out.push('\n');
        }
    }
}

fn cfg_for_build(build: &BuildBranch) -> Option<String> {
    let preds: Vec<String> = build
        .req_cargo_features
        .as_ref()
        .map(|features| {
            features
                .iter()
                .map(|f| format!("feature = \"{f}\""))
                .collect()
        })
        .unwrap_or_default();
    match preds.len() {
        0 => None,
        1 => Some(format!("#[cfg({})]", preds[0])),
        _ => Some(format!("#[cfg(all({}))]", preds.join(", "))),
    }
}
