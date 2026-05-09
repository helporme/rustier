use std::{collections::HashMap, fmt, fs, io, path::Path};

/// A representation of a crate entry (lib.rs) where it's content conditionally
/// included at build time based on the presence of cargo features, hence
/// "switch" in name.
///
///
/// Example:
/// ```
/// use rust_curse::sync::write_lib::*;
///
/// let mut switch = LibSwitch::new();
/// switch
///     .set_branch("lib", LibBranch::new("// always included"))
///     .set_branch(
///         "feature_x",
///         LibBranch::new("// feature-x\n").requires_cargo_feature("feature-x"),
///     )
///     .set_xor_branch(
///         "feature_y",
///         LibBranch::new("// feature-y\n")
///             .requires_cargo_feature("feature-y")
///             .include(),
///     );
///
/// let files = render_lib(&switch).unwrap().unwrap();
///
/// // The top-level switch wires up each branch's `{name}.rs` with a cfg
/// // gate. Default branches use `#[path] mod` so proc-macro state (e.g.
/// // napi-derive) flows correctly between items; `.include()` opts back
/// // into `include!()` for branches that must inject items at crate root.
/// let lib = files.iter().find(|f| f.name == "lib").unwrap();
/// assert_eq!(
///     lib.content,
///     "// always included\n\
///      \n\
///      #[cfg(all(feature = \"feature-x\", not(feature = \"feature-y\")))]\n\
///      #[path = \"feature_x.rs\"]\n\
///      mod feature_x;\n\
///      \n\
///      #[cfg(feature = \"feature-y\")]\n\
///      include!(\"feature_y.rs\");\n",
/// );
///
/// let feature_x = files.iter().find(|f| f.name == "feature_x").unwrap();
/// assert_eq!(feature_x.content, "// feature-x\n");
///
/// let feature_y = files.iter().find(|f| f.name == "feature_y").unwrap();
/// assert_eq!(feature_y.content, "// feature-y\n");
/// ```
#[derive(Default, Debug)]
pub struct LibSwitch {
    pub branches: HashMap<String, LibBranchKind>,
}

#[derive(Debug)]
pub enum LibBranchKind {
    Or(LibBranch),
    Xor(LibBranch),
}

impl LibBranchKind {
    fn lib(&self) -> &LibBranch {
        match self {
            LibBranchKind::Or(l) | LibBranchKind::Xor(l) => l,
        }
    }
}

impl LibSwitch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a branch that is included whenever its requirements are met,
    /// alongside any other matching branches; replaces any existing branch
    /// with the same name.
    pub fn set_branch(&mut self, name: impl Into<String>, lib: LibBranch) -> &mut Self {
        self.branches.insert(name.into(), LibBranchKind::Or(lib));
        self
    }

    /// Sets a branch that, when its requirements are met, is included to the
    /// exclusion of every other branch in the switch; replaces any existing
    /// branch with the same name.
    pub fn set_xor_branch(&mut self, name: impl Into<String>, lib: LibBranch) -> &mut Self {
        self.branches.insert(name.into(), LibBranchKind::Xor(lib));
        self
    }
}

/// A branch of a [`LibSwitch`] that is written to disk when all requirements
/// are met.
#[derive(Default, Debug)]
pub struct LibBranch {
    pub content: String,
    pub req_cargo_features: Option<Vec<String>>,
    pub req_env_flags: Option<Vec<String>>,
    pub use_include: bool,
}

impl LibBranch {
    /// Creates a new [`Lib`] with the given content.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            ..Default::default()
        }
    }

    /// Requires the given cargo feature to be enabled.
    pub fn requires_cargo_feature(mut self, feature: impl Into<String>) -> Self {
        let feature = feature.into();
        let mut req_cargo_features = self.req_cargo_features.unwrap_or_default();
        if !req_cargo_features.contains(&feature) {
            req_cargo_features.push(feature);
        }
        self.req_cargo_features = Some(req_cargo_features);
        self
    }

    /// Requires the given environment flag to be set.
    pub fn requires_env_flag(mut self, flag: impl Into<String>) -> Self {
        let flag = flag.into();
        let mut req_env_flags = self.req_env_flags.unwrap_or_default();
        if !req_env_flags.contains(&flag) {
            req_env_flags.push(flag);
        }
        self.req_env_flags = Some(req_env_flags);
        self
    }

    /// Emits `include!("<name>.rs");` for this branch instead of the default
    /// `#[path = "<name>.rs"] mod <name>;`. Use when the branch's content
    /// must land at the crate root -> e.g. `pub mod` declarations or
    /// `pub use` re-exports that downstream code references as
    /// `crate::<...>`.
    ///
    /// The default mod-based emission is preferred because it preserves
    /// proc-macro expansion order between items in the same branch
    /// (notably napi-derive, which links a `#[napi] struct` to its
    /// `#[napi] impl` via process-global state that does not flow across
    /// `include!` boundaries).
    pub fn include(mut self) -> Self {
        self.use_include = true;
        self
    }
}

impl fmt::Display for LibSwitch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.branches.is_empty() {
            return writeln!(f, "branches: []");
        }
        let mut sorted: Vec<(&String, &LibBranchKind)> = self.branches.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(b.0));
        writeln!(f, "branches:")?;
        for (name, branch) in sorted {
            let (kind, lib) = match branch {
                LibBranchKind::Or(l) => ("or", l),
                LibBranchKind::Xor(l) => ("xor", l),
            };
            write!(f, "  - {name} ({kind})")?;
            if let Some(features) = lib.req_cargo_features.as_ref().filter(|v| !v.is_empty()) {
                write!(f, " cargo_features={features:?}")?;
            }
            if let Some(flags) = lib.req_env_flags.as_ref().filter(|v| !v.is_empty()) {
                write!(f, " env_flags={flags:?}")?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

/// Error returned by [`render_lib`] when the [`LibSwitch`] is malformed.
#[derive(Debug, thiserror::Error)]
pub enum RenderLibError {
    #[error("the \"lib\" branch must be added with `set_branch(...)`, not `set_xor_branch(...)`")]
    LibBranchMustBeOr,
    #[error(
        "xor branch \"{name}\" has no requirements; an unconditional xor would suppress every other branch"
    )]
    XorBranchHasNoRequirements { name: String },
}

/// Error returned by [`write_lib`] when rendering or writing fails.
#[derive(Debug, thiserror::Error)]
pub enum WriteLibError {
    #[error(transparent)]
    Render(#[from] RenderLibError),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

/// Writes the given [`LibSwitch`] to a disk as multiple files, see the
/// [`render_lib`] documentation for more details. No-op when [`render_lib`]
/// has nothing to emit.
pub fn write_lib(dir_path: &Path, switch: &LibSwitch) -> Result<(), WriteLibError> {
    let Some(files) = render_lib(switch)? else {
        return Ok(());
    };
    for file in &files {
        let path = dir_path.join(format!("{}.rs", file.name));
        fs::write(path, &file.content)?;
    }
    Ok(())
}

/// Rendered file content.
#[derive(Debug)]
pub struct RenderedFile {
    /// The rendered file content.
    pub content: String,
    /// The name of the file with the `.rs` extension.
    /// E.g. `lib.rs`, `my-feature.rs`, etc.
    pub name: String,
}

/// Renders the given [`LibSwitch`] into a bunch of [`RenderedFile`]s.
///
/// Each branch is wired into the "switch" lib file with a `#[cfg(...)]`
/// gate built from its requirements. By default a branch is emitted as
/// `#[path = "<name>.rs"] mod <name>;`; branches that opt in via
/// [`LibBranch::include`] are emitted as `include!("<name>.rs");`
/// instead, for cases where the branch's items must live at the crate
/// root.
///
/// Branches whose content is empty are skipped entirely: no helper file is
/// emitted and no wiring line is added to the switch lib.
///
/// All rendered lib files should be written to the disk in the same directory
/// as the caller, file names are derived from the branch names. "switch" lib
/// file is always named lib.rs. Returns [`None`] when there is nothing to
/// emit (no non-empty branches and no "lib" prelude).
pub fn render_lib(switch: &LibSwitch) -> Result<Option<Vec<RenderedFile>>, RenderLibError> {
    if switch.branches.is_empty() {
        return Ok(None);
    }
    if let Some(LibBranchKind::Xor(_)) = switch.branches.get("lib") {
        return Err(RenderLibError::LibBranchMustBeOr);
    }
    for (name, branch) in &switch.branches {
        if branch.lib().content.is_empty() {
            continue;
        }
        if let LibBranchKind::Xor(lib) = branch {
            if requirements(lib).is_empty() {
                return Err(RenderLibError::XorBranchHasNoRequirements { name: name.clone() });
            }
        }
    }

    let suppress = suppress_condition(switch);

    let mut sorted: Vec<(&String, &LibBranchKind)> = switch
        .branches
        .iter()
        .filter(|(name, branch)| name.as_str() != "lib" && !branch.lib().content.is_empty())
        .collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    let lib_prelude = switch
        .branches
        .get("lib")
        .map(|b| b.lib().content.as_str())
        .unwrap_or("");

    if sorted.is_empty() && lib_prelude.is_empty() {
        return Ok(None);
    }

    let mut lib_content = String::new();
    if !lib_prelude.is_empty() {
        lib_content.push_str(lib_prelude);
        if !lib_prelude.ends_with('\n') {
            lib_content.push('\n');
        }
    }

    let prelude_present = !lib_content.is_empty();
    let mut files = Vec::with_capacity(sorted.len() + 1);

    for (i, (name, branch)) in sorted.iter().enumerate() {
        if i > 0 || prelude_present {
            lib_content.push('\n');
        }
        if let Some(cfg) = cfg_for_branch(branch, suppress.as_deref()) {
            lib_content.push_str(&cfg);
            lib_content.push('\n');
        }
        if branch.lib().use_include {
            lib_content.push_str(&format!("include!(\"{name}.rs\");\n"));
        } else {
            lib_content.push_str(&format!("#[path = \"{name}.rs\"]\nmod {name};\n"));
        }

        files.push(RenderedFile {
            name: (*name).clone(),
            content: branch.lib().content.clone(),
        });
    }

    files.push(RenderedFile {
        name: "lib".to_string(),
        content: lib_content,
    });

    Ok(Some(files))
}

fn requirements(lib: &LibBranch) -> Vec<String> {
    let mut preds = Vec::new();
    if let Some(features) = &lib.req_cargo_features {
        for f in features {
            preds.push(format!("feature = \"{f}\""));
        }
    }
    if let Some(flags) = &lib.req_env_flags {
        for f in flags {
            preds.push(f.clone());
        }
    }
    preds
}

fn suppress_condition(switch: &LibSwitch) -> Option<String> {
    let mut xor_preds: Vec<String> = Vec::new();
    for branch in switch.branches.values() {
        if let LibBranchKind::Xor(lib) = branch {
            if lib.content.is_empty() {
                continue;
            }
            let preds = requirements(lib);
            match preds.len() {
                0 => continue,
                1 => xor_preds.push(preds.into_iter().next().unwrap()),
                _ => xor_preds.push(format!("all({})", preds.join(", "))),
            }
        }
    }
    xor_preds.sort();
    match xor_preds.len() {
        0 => None,
        1 => Some(xor_preds.into_iter().next().unwrap()),
        _ => Some(format!("any({})", xor_preds.join(", "))),
    }
}

fn cfg_for_branch(branch: &LibBranchKind, suppress: Option<&str>) -> Option<String> {
    let mut preds = requirements(branch.lib());
    if matches!(branch, LibBranchKind::Or(_)) {
        if let Some(s) = suppress {
            preds.push(format!("not({s})"));
        }
    }
    match preds.len() {
        0 => None,
        1 => Some(format!("#[cfg({})]", preds[0])),
        _ => Some(format!("#[cfg(all({}))]", preds.join(", "))),
    }
}
