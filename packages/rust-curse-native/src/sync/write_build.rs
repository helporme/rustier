use std::{collections::HashMap, path::Path};

/// A representation of a crate build.rs where it's content conditionally
/// included at build time based on the presence of cargo features, hence
/// "switch" in name.
///
///
/// Example:
/// ```no_run
/// use rust_curse::sync::write_build::*;
///
/// let switch = BuildSwitch::new()
///     .branch("default", Build::new("// always included"))
///     .branch(
///         "feature_x",
///         Build::new("// feature-x\n").requires_cargo_feature("feature-x"),
///     )
///
/// let files = render_build(&switch).unwrap();
///
/// // The top-level switch includes each branch's `build-{name}.rs`,
/// // gated by the branch's requirements.
/// let build = files.iter().find(|f| f.name == "default").unwrap();
/// assert_eq!(
///     build.content,
///     // UPDATE ME!
/// );
///
/// let feature_x = files.iter().find(|f| f.name == "feature_x").unwrap();
/// assert_eq!(feature_x.content, "UPDATE ME!");
#[derive(Default, Debug)]
pub struct BuildSwitch {
    pub branches: HashMap<String, Build>,
}

impl BuildSwitch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a Build content which is always applied when it's requirements are
    /// met.
    pub fn branch(mut self, name: impl Into<String>, build: Build) -> Self {
        self.branches.insert(name.into(), build);
        self
    }
}

/// A branch of a [`BuildSwitch`] that is written to disk when all requirements
/// are met.
#[derive(Default, Debug)]
pub struct Build {
    pub preamble: String,
    pub body: String,
    pub req_cargo_features: Option<Vec<String>>,
}

impl Build {
    /// Creates a new [`Build`] with the given preamble and body.
    pub fn new(preamble: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            preamble: preamble.into(),
            body: body.into(),
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

/// Writes the given [`BuildSwitch`] to a disk as multiple files, see the
/// [`render_build`] documentation for more details.
pub fn write_build(dir_path: &Path, switch: &BuildSwitch) -> Result<(), u8> {
    let content = render_build(switch)?;
    // write files
    Ok(())
}

/// Rendered file content.
pub struct RenderedFile {
    /// The rendered file content.
    pub content: String,
    /// The name of the file with the `.rs` extension.
    /// E.g. `Build.rs`, `my-feature.rs`, etc.
    pub name: String,
}

/// Renders the given [`BuildSwitch`] into a bunch of [`RenderedFile`]s.
///
/// Utilizes the include!() with #[cfg] directives to conditionally include
/// Build.rs content based on the presence of cargo features / env flags. For
/// include!() to work inside "switch" Build file, multiple "helper" Build files
/// are used to define the content to include, hence multiple [`RenderedFile`]s
/// are returned.
///
/// All rendered Build files should be written to the disk in the same directory
/// as the caller, file names are derived from the branch names. "switch" Build
/// file is always named Build.rs.
pub fn render_build(switch: &BuildSwitch) -> Result<Vec<RenderedFile>, u8> {
    todo!()
}
