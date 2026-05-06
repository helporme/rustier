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
/// let switch = LibSwitch::new()
///     .branch("lib", Lib::new("// always included"))
///     .branch(
///         "feature_x",
///         Lib::new("// feature-x\n").requires_cargo_feature("feature-x"),
///     )
///     .xor_branch(
///         "feature_y",
///         Lib::new("// feature-y\n").requires_cargo_feature("feature-y"),
///     );
///
/// let files = render_lib(&switch).unwrap();
///
/// // The top-level switch includes each branch's `{name}.rs`,
/// // gated by the branch's requirements.
/// let lib = files.iter().find(|f| f.name == "lib").unwrap();
/// assert_eq!(
///     lib.content,
///     "// always included\n\
///      \n\
///      #[cfg(all(feature = \"feature-x\", not(feature = \"feature-y\")))]\n\
///      include!(\"feature_x.rs\");\n\
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
    pub branches: HashMap<String, LibBranch>,
}

#[derive(Debug)]
pub enum LibBranch {
    Or(Lib),
    Xor(Lib),
}

impl LibBranch {
    fn lib(&self) -> &Lib {
        match self {
            LibBranch::Or(l) | LibBranch::Xor(l) => l,
        }
    }
}

impl LibSwitch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a lib content which is always applied when it's requirements are
    /// met.
    pub fn branch(mut self, name: impl Into<String>, lib: Lib) -> Self {
        self.branches.insert(name.into(), LibBranch::Or(lib));
        self
    }

    /// Adds a lib which is _exclusively_ applied when it's requirements are
    /// met. It means that if this lib is applied, no other lib will be
    /// applied.
    pub fn xor_branch(mut self, name: impl Into<String>, lib: Lib) -> Self {
        self.branches.insert(name.into(), LibBranch::Xor(lib));
        self
    }
}

/// A branch of a [`LibSwitch`] that is written to disk when all requirements
/// are met.
#[derive(Default, Debug)]
pub struct Lib {
    pub content: String,
    pub req_cargo_features: Option<Vec<String>>,
    pub req_env_flags: Option<Vec<String>>,
}

impl Lib {
    /// Creates a new [`Lib`] with the given content.
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

    /// Requires the given environment flag to be set.
    pub fn requires_env_flag(mut self, flag: impl Into<String>) -> Self {
        let mut req_env_flags = self.req_env_flags.unwrap_or_default();
        req_env_flags.push(flag.into());
        self.req_env_flags = Some(req_env_flags);
        self
    }
}

impl fmt::Display for LibSwitch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.branches.is_empty() {
            return writeln!(f, "branches: []");
        }
        let mut sorted: Vec<(&String, &LibBranch)> = self.branches.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(b.0));
        writeln!(f, "branches:")?;
        for (name, branch) in sorted {
            let (kind, lib) = match branch {
                LibBranch::Or(l) => ("or", l),
                LibBranch::Xor(l) => ("xor", l),
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
    #[error("the \"lib\" branch must be added with `branch(...)`, not `xor_branch(...)`")]
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
/// [`render_lib`] documentation for more details.
pub fn write_lib(dir_path: &Path, switch: &LibSwitch) -> Result<(), WriteLibError> {
    let files = render_lib(switch)?;
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
/// Utilizes the include!() with #[cfg] directives to conditionally include
/// lib.rs content based on the presence of cargo features / env flags. For
/// include!() to work inside "switch" lib file, multiple "helper" lib files are
/// used to define the content to include, hence multiple [`RenderedFile`]s are
/// returned.
///
/// All rendered lib files should be written to the disk in the same directory
/// as the caller, file names are derived from the branch names. "switch" lib
/// file is always named lib.rs.
pub fn render_lib(switch: &LibSwitch) -> Result<Vec<RenderedFile>, RenderLibError> {
    if let Some(LibBranch::Xor(_)) = switch.branches.get("lib") {
        return Err(RenderLibError::LibBranchMustBeOr);
    }
    for (name, branch) in &switch.branches {
        if let LibBranch::Xor(lib) = branch {
            if requirements(lib).is_empty() {
                return Err(RenderLibError::XorBranchHasNoRequirements { name: name.clone() });
            }
        }
    }

    let suppress = suppress_condition(switch);

    let mut sorted: Vec<(&String, &LibBranch)> = switch
        .branches
        .iter()
        .filter(|(name, _)| name.as_str() != "lib")
        .collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    let lib_prelude = switch
        .branches
        .get("lib")
        .map(|b| b.lib().content.as_str())
        .unwrap_or("");

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
        lib_content.push_str(&format!("include!(\"{name}.rs\");\n"));

        files.push(RenderedFile {
            name: (*name).clone(),
            content: branch.lib().content.clone(),
        });
    }

    files.push(RenderedFile {
        name: "lib".to_string(),
        content: lib_content,
    });

    Ok(files)
}

fn requirements(lib: &Lib) -> Vec<String> {
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
        if let LibBranch::Xor(lib) = branch {
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

fn cfg_for_branch(branch: &LibBranch, suppress: Option<&str>) -> Option<String> {
    let mut preds = requirements(branch.lib());
    if matches!(branch, LibBranch::Or(_)) {
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
