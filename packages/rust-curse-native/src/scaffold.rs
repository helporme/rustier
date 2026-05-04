//! Scaffolding for the `.rust-curse/` output directory.
//!
//! A [`Scaffold`] manages the `.rust-curse/` folder and Cargo.toml inside the
//! user's project and it's composed of named [`ScaffoldSection`]s. Each section
//! specifies its own activation (when it should be active), the code it
//! contributes to the generated crate, and the dependencies it needs.
//!
//! ```no_run
//! use rust_curse::scaffold::napi::NapiSectionOpts;
//! use rust_curse::scaffold::rustdoc::RustdocSectionOpts;
//! use rust_curse::scaffold::{Scaffold, ScaffoldOptions};
//! use std::path::PathBuf;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut scaffold = Scaffold::new(ScaffoldOptions {
//!     abs_root_path: "/path/to/project".into(),
//!     temp_folder_rel_path: ".rust-curse".into(),
//!     cargo_toml_rel_path: "Cargo.toml".into(),
//! })?;
//!
//! scaffold.set_napi(NapiSectionOpts {
//!     content: "// generated napi glue\n".into(),
//! })?;
//! scaffold.set_rustdoc(RustdocSectionOpts { modules: &[] })?;
//!
//! scaffold.deploy()?;
//! # Ok(())
//! # }
//! ```
//!
//! For lower-level control, build a [`ScaffoldSection`] directly and
//! feed it through [`Scaffold::set`] / [`Scaffold::patch`].

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

mod cargo_patch;
mod dispatcher;
pub mod link;
mod meta;
pub mod napi;
pub mod rustdoc;
mod tree;
pub mod ts;
pub mod wasm;

#[derive(Debug, Clone)]
pub struct ScaffoldOptions {
    /// Absolute path to the project dir root, e.g. to the folder that contains
    /// `package.json`
    pub abs_root_path: PathBuf,
    /// Path to the scaffold output dir (`.rust-curse`), relative to
    /// `abs_root_path`.
    pub temp_folder_rel_path: PathBuf,
    // Path to the `Cargo.toml`, relative to `abs_root_path`.
    pub cargo_toml_rel_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Activation {
    /// Active when a Cargo feature is enabled. Adds to `[features]` and
    /// gates with `#[cfg(feature = "...")]` in the dispatcher.
    CargoFeature(String),

    /// Active when an env var is set during `cargo build`, and exclusively
    /// so: every other section in the scaffold is suppressed while this
    /// section is active. The build dispatcher emits
    /// `cargo:rustc-cfg=<cfg_flag>` when the env var is present; the lib
    /// dispatcher gates this section with `#[cfg(<cfg_flag>)]` and gates
    /// every non-exclusive section with `#[cfg(not(<cfg_flag>))]`.
    ///
    /// At most one section per [`Scaffold`] may use this activation;
    /// adding a second returns [`ScaffoldError::MultipleExclusiveEnv`].
    ExclusiveEnv { env_var: String, cfg_flag: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dep {
    pub name: String,
    pub version: String,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildContribution {
    /// Code emitted at the crate root of this section's `build-{name}.rs`,
    /// e.g. `use napi_build;`. Caller controls newlines.
    pub preamble: String,
    /// Code emitted inside `pub fn setup()` of this section's
    /// `build-{name}.rs`, e.g. `napi_build::setup();`. Caller controls
    /// newlines; lines are auto-indented inside the setup body.
    pub main: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum Field<T> {
    #[default]
    Untouched,
    Set(T),
}

#[derive(Debug, Default)]
pub struct ScaffoldSection {
    pub(crate) name: String,
    pub(crate) activation: Field<Option<Activation>>,
    pub(crate) content: Field<String>,
    pub(crate) build_content: Field<BuildContribution>,
    pub(crate) deps: Field<Vec<Dep>>,
    pub(crate) build_deps: Field<Vec<Dep>>,
    pub(crate) external_files: Field<Vec<String>>,
}

impl ScaffoldSection {
    /// Start a new section with the given name.
    ///
    /// A section without an activation applies unconditionally — it is
    /// always linked in by the dispatcher, unless suppressed by a section
    /// with [`exclusive_env_flag`](Self::exclusive_env_flag).
    ///
    /// ```
    /// use rust_curse::scaffold::ScaffoldSection;
    ///
    /// let section = ScaffoldSection::new("napi");
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Activate this section when the named cargo feature is enabled.
    ///
    /// ```
    /// use rust_curse::scaffold::ScaffoldSection;
    ///
    /// let napi = ScaffoldSection::new("napi")
    ///     .cargo_feature_flag("node")
    ///     .content("// napi glue\n");
    /// ```
    pub fn cargo_feature_flag(mut self, name: impl Into<String>) -> Self {
        self.activation = Field::Set(Some(Activation::CargoFeature(name.into())));
        self
    }

    /// Activate this section when the given env var is set during
    /// `cargo build`, and *exclusively* so — when this section is active,
    /// every other section in the scaffold is suppressed.
    ///
    /// At most one section may use this; a second call to
    /// [`Scaffold::set`] with another exclusive section returns
    /// [`ScaffoldError::MultipleExclusiveEnv`].
    ///
    /// ```
    /// use rust_curse::scaffold::ScaffoldSection;
    ///
    /// let rustdoc = ScaffoldSection::new("rustdoc")
    ///     .exclusive_env_flag("RUST_CURSE_RUSTDOC")
    ///     .content("pub mod foo;\n");
    /// // Active when `RUST_CURSE_RUSTDOC=1 cargo build`; everything else
    /// // is gated off via `#[cfg(not(rust_curse_rustdoc))]`.
    /// ```
    pub fn exclusive_env_flag(mut self, env_var: impl Into<String>) -> Self {
        let env_var = env_var.into();
        let cfg_flag = env_var.to_lowercase();
        self.activation = Field::Set(Some(Activation::ExclusiveEnv { env_var, cfg_flag }));
        self
    }

    /// Set the body of `lib-{name}.rs` for this section.
    ///
    /// ```
    /// use rust_curse::scaffold::ScaffoldSection;
    ///
    /// let section = ScaffoldSection::new("napi")
    ///     .cargo_feature_flag("node")
    ///     .content("pub fn add(a: i32, b: i32) -> i32 { a + b }\n");
    /// ```
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Field::Set(content.into());
        self
    }

    /// Contribute `build-{name}.rs` code for this section.
    /// ```
    /// use rust_curse::scaffold::{BuildContribution, ScaffoldSection};
    ///
    /// let napi = ScaffoldSection::new("napi")
    ///     .cargo_feature_flag("node")
    ///     .build_content(BuildContribution {
    ///         preamble: "use napi_build;".into(),
    ///         main: "napi_build::setup();".into(),
    ///     });
    /// ```
    pub fn build_content(mut self, contrib: BuildContribution) -> Self {
        self.build_content = Field::Set(contrib);
        self
    }

    /// Add a regular dependency required by this section's lib content.
    ///
    /// Multiple `dep` calls accumulate.
    /// ```
    /// use rust_curse::scaffold::ScaffoldSection;
    ///
    /// let napi = ScaffoldSection::new("napi")
    ///     .cargo_feature_flag("node")
    ///     .dep("napi", "3")
    ///     .dep("napi-derive", "3");
    /// ```
    pub fn dep(mut self, name: &str, version: &str) -> Self {
        let new = Dep {
            name: name.into(),
            version: version.into(),
            features: vec![],
        };
        self.deps = match self.deps {
            Field::Set(mut v) => {
                v.push(new);
                Field::Set(v)
            }
            Field::Untouched => Field::Set(vec![new]),
        };
        self
    }

    /// Like [`dep`](Self::dep), but enables specific cargo features on
    /// the dependency.
    ///
    /// ```
    /// use rust_curse::scaffold::ScaffoldSection;
    ///
    /// let wasm = ScaffoldSection::new("wasm")
    ///     .cargo_feature_flag("wasm")
    ///     .dep_with_features("serde", "1", &["derive"]);
    /// ```
    pub fn dep_with_features(mut self, name: &str, version: &str, features: &[&str]) -> Self {
        let new = Dep {
            name: name.into(),
            version: version.into(),
            features: features.iter().map(|s| s.to_string()).collect(),
        };
        self.deps = match self.deps {
            Field::Set(mut v) => {
                v.push(new);
                Field::Set(v)
            }
            Field::Untouched => Field::Set(vec![new]),
        };
        self
    }

    /// Add a build dependency required by this section's build.rs.
    ///
    /// ```
    /// use rust_curse::scaffold::{BuildContribution, ScaffoldSection};
    ///
    /// let napi = ScaffoldSection::new("napi")
    ///     .cargo_feature_flag("node")
    ///     .build_content(BuildContribution {
    ///         preamble: "use napi_build;".into(),
    ///         main: "napi_build::setup();".into(),
    ///     })
    ///     .build_dep("napi-build", "2");
    /// ```
    pub fn build_dep(mut self, name: &str, version: &str) -> Self {
        let new = Dep {
            name: name.into(),
            version: version.into(),
            features: vec![],
        };
        self.build_deps = match self.build_deps {
            Field::Set(mut v) => {
                v.push(new);
                Field::Set(v)
            }
            Field::Untouched => Field::Set(vec![new]),
        };
        self
    }

    /// Declare a path (relative to the scaffold root) that this section
    /// owns but writes via other tooling. Multiple `external` calls
    /// accumulate.
    ///
    /// On `set` and `remove`, scaffold deletes any previously declared paths
    /// that are no longer in the list. Paths must be relative and may
    /// not contain `..`.
    ///
    /// ```
    /// use rust_curse::scaffold::ScaffoldSection;
    ///
    /// let wasm = ScaffoldSection::new("wasm")
    ///     .cargo_feature_flag("wasm")
    ///     .external("wasm/")
    ///     .external("pkg/index.js");
    /// ```
    pub fn external(mut self, rel_path: &str) -> Self {
        let new = rel_path.to_string();
        self.external_files = match self.external_files {
            Field::Set(mut v) => {
                v.push(new);
                Field::Set(v)
            }
            Field::Untouched => Field::Set(vec![new]),
        };
        self
    }
}

/// Fully-resolved section state held in memory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Section {
    pub activation: Option<Activation>,
    pub content: Option<String>,
    pub build_content: Option<BuildContribution>,
    pub deps: Vec<Dep>,
    pub build_deps: Vec<Dep>,
    pub external_files: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ScaffoldError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid external_files path '{0}': must be relative and not contain '..'")]
    PathEscape(String),
    #[error("section '{0}' has not been set; call set() before patch()")]
    PatchUnknownSection(String),
    #[error(
        "section '{attempted}' uses exclusive_env_flag, but section '{existing}' already does; \
         only one exclusive_env_flag section is allowed per scaffold"
    )]
    MultipleExclusiveEnv { existing: String, attempted: String },
    #[error("invalid Cargo.toml: {0}")]
    CargoToml(#[from] toml_edit::TomlError),
    #[error("invalid meta.json: {0}")]
    Meta(#[from] serde_json::Error),
}

pub struct Scaffold {
    abs_root_path: PathBuf,
    abs_temp_path: PathBuf,
    abs_cargo_toml_path: PathBuf,
    sections: BTreeMap<String, Section>,
    prev_meta: meta::Meta,
}

impl Scaffold {
    /// Open a scaffold rooted at `abs_root_path`.
    ///
    /// ```no_run
    /// use rust_curse::scaffold::{Scaffold, ScaffoldOptions};
    /// use std::path::PathBuf;
    ///
    /// // Top-level scaffold dir.
    /// let scaffold = Scaffold::new(ScaffoldOptions {
    ///     abs_root_path: "/path/to/project".into(),
    ///     temp_folder_rel_path: ".rust-curse".into(),
    ///     cargo_toml_rel_path: "Cargo.toml".into(),
    /// })
    /// .unwrap();
    /// ```
    pub fn new(opts: ScaffoldOptions) -> Result<Self, ScaffoldError> {
        let abs_temp_path = opts.abs_root_path.join(&opts.temp_folder_rel_path);
        let abs_cargo_toml_path = opts.abs_root_path.join(&opts.cargo_toml_rel_path);

        fs::create_dir_all(&abs_temp_path).map_err(|e| ScaffoldError::Io {
            path: abs_temp_path.clone(),
            source: e,
        })?;

        let prev_meta = meta::Meta::load(&abs_temp_path)?;

        Ok(Self {
            abs_root_path: opts.abs_root_path,
            abs_temp_path,
            abs_cargo_toml_path,
            sections: BTreeMap::new(),
            prev_meta,
        })
    }

    /// Insert or replace a section in the in-memory catalog.
    ///
    /// Use [`patch`](Self::patch) to update a subset of fields. Nothing is
    /// written to disk until [`deploy`](Self::deploy) is called.
    ///
    /// ```no_run
    /// # use std::path::PathBuf;
    /// # use rust_curse::scaffold::{Scaffold, ScaffoldOptions, ScaffoldSection};
    /// # let mut scaffold = Scaffold::new(ScaffoldOptions {
    /// #     abs_root_path: PathBuf::from("/tmp/example"),
    /// #     temp_folder_rel_path: ".rust-curse".into(),
    /// #     cargo_toml_rel_path: PathBuf::from("Cargo.toml"),
    /// # }).unwrap();
    /// scaffold
    ///     .set(
    ///         ScaffoldSection::new("napi")
    ///             .cargo_feature_flag("node")
    ///             .content("// generated\n"),
    ///     )
    ///     .unwrap();
    /// ```
    pub fn set(&mut self, section: ScaffoldSection) -> Result<(), ScaffoldError> {
        for ef in iter_external_files(&section) {
            validate_external_path(ef)?;
        }
        let resolved = resolve_set(&section);
        if matches!(resolved.activation, Some(Activation::ExclusiveEnv { .. })) {
            if let Some(existing) = self.find_exclusive_env(&section.name) {
                return Err(ScaffoldError::MultipleExclusiveEnv {
                    existing,
                    attempted: section.name,
                });
            }
        }
        self.sections.insert(section.name, resolved);
        Ok(())
    }

    fn find_exclusive_env(&self, ignore_name: &str) -> Option<String> {
        self.sections
            .iter()
            .find(|(name, s)| {
                name.as_str() != ignore_name
                    && matches!(s.activation, Some(Activation::ExclusiveEnv { .. }))
            })
            .map(|(name, _)| name.clone())
    }

    /// Update only the fields that were explicitly set on `section`,
    /// leaving every other field of the existing section untouched.
    ///
    /// ```no_run
    /// # use std::path::PathBuf;
    /// # use rust_curse::scaffold::{Scaffold, ScaffoldOptions, ScaffoldSection};
    /// # use rust_curse::scaffold::napi::NapiSectionOpts;
    /// # let mut scaffold = Scaffold::new(ScaffoldOptions {
    /// #     abs_root_path: PathBuf::from("/tmp/example"),
    /// #     temp_folder_rel_path: ".rust-curse".into(),
    /// #     cargo_toml_rel_path: PathBuf::from("Cargo.toml"),
    /// # }).unwrap();
    /// scaffold
    ///     .set(
    ///         ScaffoldSection::new("napi")
    ///             .cargo_feature_flag("node")
    ///             .content("// generated\n"),
    ///     )
    ///     .unwrap();
    ///
    /// scaffold
    ///     .patch(ScaffoldSection::new("napi").content("// updated\n"))
    ///     .unwrap();
    /// ```
    pub fn patch(&mut self, section: ScaffoldSection) -> Result<(), ScaffoldError> {
        for ef in iter_external_files(&section) {
            validate_external_path(ef)?;
        }
        let existing = self
            .sections
            .get_mut(&section.name)
            .ok_or_else(|| ScaffoldError::PatchUnknownSection(section.name.clone()))?;
        apply_patch(section, existing);
        Ok(())
    }

    /// Drop a section from the in-memory catalog. On the next `deploy`,
    /// removes everything associated with the section.
    ///
    /// ```no_run
    /// # use std::path::PathBuf;
    /// # use rust_curse::scaffold::{Scaffold, ScaffoldOptions};
    /// # let mut scaffold = Scaffold::new(ScaffoldOptions {
    /// #     abs_root_path: PathBuf::from("/tmp/example"),
    /// #     temp_folder_rel_path: ".rust-curse".into(),
    /// #     cargo_toml_rel_path: PathBuf::from("Cargo.toml"),
    /// # }).unwrap();
    /// scaffold.remove("napi").unwrap();
    /// ```
    pub fn remove(&mut self, name: &str) -> Result<(), ScaffoldError> {
        self.sections.remove(name);
        Ok(())
    }

    /// Resolve a path relative to the scaffold root (e.g. `.rust-curse/`).
    ///
    /// ```no_run
    /// # use std::path::PathBuf;
    /// # use rust_curse::scaffold::{Scaffold, ScaffoldOptions};
    /// # let scaffold = Scaffold::new(ScaffoldOptions {
    /// #     abs_root_path: PathBuf::from("/tmp/example"),
    /// #     temp_folder_rel_path: ".rust-curse".into(),
    /// #     cargo_toml_rel_path: PathBuf::from("Cargo.toml"),
    /// # }).unwrap();
    /// let wasm_dir = scaffold.abs_path_to("wasm/");
    /// // -> /tmp/example/.rust-curse/wasm/
    /// ```
    pub fn abs_path_to(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.abs_temp_path.join(rel)
    }

    /// Resolve a path relative to the project root (sibling of the
    /// scaffold dir).
    ///
    /// ```no_run
    /// # use std::path::PathBuf;
    /// # use rust_curse::scaffold::{Scaffold, ScaffoldOptions};
    /// # let scaffold = Scaffold::new(ScaffoldOptions {
    /// #     abs_root_path: PathBuf::from("/tmp/example"),
    /// #     temp_folder_rel_path: ".rust-curse".into(),
    /// #     cargo_toml_rel_path: PathBuf::from("Cargo.toml"),
    /// # }).unwrap();
    /// let tsconfig = scaffold.abs_path_from_project_root_to("tsconfig.json");
    /// // -> /tmp/example/tsconfig.json
    /// ```
    pub fn abs_path_from_project_root_to(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.abs_root_path.join(rel)
    }

    /// Write everything to disk and reconcile:
    ///
    /// ```no_run
    /// # use std::path::PathBuf;
    /// # use rust_curse::scaffold::{Scaffold, ScaffoldOptions, ScaffoldSection};
    /// let mut scaffold = Scaffold::new(ScaffoldOptions {
    ///     abs_root_path: "/tmp/example".into(),
    ///     temp_folder_rel_path: ".rust-curse".into(),
    ///     cargo_toml_rel_path: "Cargo.toml".into(),
    /// })
    /// .unwrap();
    ///
    /// scaffold
    ///     .set(
    ///         ScaffoldSection::new("example")
    ///             .cargo_feature_flag("example")
    ///             .content("// generated\n"),
    ///     )
    ///     .unwrap();
    ///
    /// scaffold.deploy().unwrap();
    /// ```
    pub fn deploy(&mut self) -> Result<(), ScaffoldError> {
        let prev_names: std::collections::BTreeSet<&str> =
            self.prev_meta.sections.keys().map(String::as_str).collect();
        let now_names: std::collections::BTreeSet<&str> =
            self.sections.keys().map(String::as_str).collect();

        for stale_name in prev_names.difference(&now_names) {
            self.delete_section_file(&format!("lib-{stale_name}.rs"))?;
            self.delete_section_file(&format!("build-{stale_name}.rs"))?;
            if let Some(prev) = self.prev_meta.sections.get(*stale_name) {
                self.cleanup_external_files(&prev.external_files)?;
            }
        }

        for (name, section) in &self.sections {
            if let Some(prev) = self.prev_meta.sections.get(name) {
                let prev_set: std::collections::BTreeSet<&String> =
                    prev.external_files.iter().collect();
                let new_set: std::collections::BTreeSet<&String> =
                    section.external_files.iter().collect();
                let stale: Vec<String> = prev_set
                    .difference(&new_set)
                    .map(|s| (*s).clone())
                    .collect();
                self.cleanup_external_files(&stale)?;
            }

            let lib_path = self.abs_temp_path.join(format!("lib-{name}.rs"));
            match &section.content {
                Some(content) => self.write_file(&lib_path, content)?,
                None => self.delete_if_exists(&lib_path)?,
            }

            let build_path = self.abs_temp_path.join(format!("build-{name}.rs"));
            match &section.build_content {
                Some(contrib) => {
                    let content = dispatcher::render_section_build(contrib);
                    self.write_file(&build_path, &content)?;
                }
                None => self.delete_if_exists(&build_path)?,
            }
        }

        let lib_dispatcher = dispatcher::render_lib_dispatcher(&self.sections);
        self.write_file(&self.abs_temp_path.join("lib.rs"), &lib_dispatcher)?;

        let build_path = self.abs_temp_path.join("build.rs");
        let needs_build = dispatcher::needs_build_rs(&self.sections);
        if needs_build {
            let build_dispatcher = dispatcher::render_build_dispatcher(&self.sections);
            self.write_file(&build_path, &build_dispatcher)?;
        } else {
            self.delete_if_exists(&build_path)?;
        }

        let cargo_content =
            fs::read_to_string(&self.abs_cargo_toml_path).map_err(|e| ScaffoldError::Io {
                path: self.abs_cargo_toml_path.clone(),
                source: e,
            })?;
        let abs_cargo_dir = self
            .abs_cargo_toml_path
            .parent()
            .expect("Cargo.toml has a parent directory");
        let temp_rel_to_cargo = diff_paths(&self.abs_temp_path, abs_cargo_dir);
        let lib_rel = temp_rel_to_cargo
            .join("lib.rs")
            .to_string_lossy()
            .into_owned();
        let build_rel = needs_build.then(|| {
            temp_rel_to_cargo
                .join("build.rs")
                .to_string_lossy()
                .into_owned()
        });

        let patched = cargo_patch::apply(
            &cargo_content,
            &lib_rel,
            build_rel.as_deref(),
            &self.sections,
            &self.prev_meta,
        )?;
        self.write_file(&self.abs_cargo_toml_path, &patched.content)?;

        let new_meta = meta::Meta::from_state(&self.sections, patched.cargo_state);
        new_meta.save(&self.abs_temp_path)?;
        self.prev_meta = new_meta;

        Ok(())
    }

    fn delete_section_file(&self, name: &str) -> Result<(), ScaffoldError> {
        self.delete_if_exists(&self.abs_temp_path.join(name))
    }

    fn cleanup_external_files(&self, paths: &[String]) -> Result<(), ScaffoldError> {
        for p in paths {
            let abs = self.abs_temp_path.join(p);
            self.delete_path(&abs)?;
        }
        Ok(())
    }

    fn delete_if_exists(&self, path: &Path) -> Result<(), ScaffoldError> {
        if path.exists() {
            fs::remove_file(path).map_err(|e| ScaffoldError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;
        }
        Ok(())
    }

    fn delete_path(&self, path: &Path) -> Result<(), ScaffoldError> {
        if !path.exists() {
            return Ok(());
        }
        let meta = fs::symlink_metadata(path).map_err(|e| ScaffoldError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        if meta.is_dir() {
            fs::remove_dir_all(path).map_err(|e| ScaffoldError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;
        } else {
            fs::remove_file(path).map_err(|e| ScaffoldError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;
        }
        Ok(())
    }

    fn write_file(&self, abs_path: &Path, content: &str) -> Result<(), ScaffoldError> {
        if let Some(parent) = abs_path.parent() {
            fs::create_dir_all(parent).map_err(|e| ScaffoldError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        fs::write(abs_path, content).map_err(|e| ScaffoldError::Io {
            path: abs_path.to_path_buf(),
            source: e,
        })?;
        Ok(())
    }
}

fn iter_external_files(section: &ScaffoldSection) -> &[String] {
    match &section.external_files {
        Field::Set(v) => v.as_slice(),
        Field::Untouched => &[],
    }
}

fn validate_external_path(path: &str) -> Result<(), ScaffoldError> {
    let p = Path::new(path);
    if p.is_absolute() {
        return Err(ScaffoldError::PathEscape(path.to_string()));
    }
    for comp in p.components() {
        if matches!(comp, std::path::Component::ParentDir) {
            return Err(ScaffoldError::PathEscape(path.to_string()));
        }
    }
    Ok(())
}

fn resolve_set(section: &ScaffoldSection) -> Section {
    Section {
        activation: match &section.activation {
            Field::Set(a) => a.clone(),
            Field::Untouched => None,
        },
        content: match &section.content {
            Field::Set(s) => Some(s.clone()),
            Field::Untouched => None,
        },
        build_content: match &section.build_content {
            Field::Set(b) => Some(b.clone()),
            Field::Untouched => None,
        },
        deps: match &section.deps {
            Field::Set(v) => v.clone(),
            Field::Untouched => Vec::new(),
        },
        build_deps: match &section.build_deps {
            Field::Set(v) => v.clone(),
            Field::Untouched => Vec::new(),
        },
        external_files: match &section.external_files {
            Field::Set(v) => v.clone(),
            Field::Untouched => Vec::new(),
        },
    }
}

fn apply_patch(section: ScaffoldSection, existing: &mut Section) {
    if let Field::Set(a) = section.activation {
        existing.activation = a;
    }
    if let Field::Set(s) = section.content {
        existing.content = Some(s);
    }
    if let Field::Set(b) = section.build_content {
        existing.build_content = Some(b);
    }
    if let Field::Set(v) = section.deps {
        existing.deps = v;
    }
    if let Field::Set(v) = section.build_deps {
        existing.build_deps = v;
    }
    if let Field::Set(v) = section.external_files {
        existing.external_files = v;
    }
}

/// Compute a relative path from `abs_from` to `abs_to`. Both must be
/// absolute and canonicalised.
fn diff_paths(abs_to: &Path, abs_from: &Path) -> PathBuf {
    let mut from_iter = abs_from.components().peekable();
    let mut to_iter = abs_to.components().peekable();

    while let (Some(a), Some(b)) = (from_iter.peek(), to_iter.peek()) {
        if a != b {
            break;
        }
        from_iter.next();
        to_iter.next();
    }

    let mut result = PathBuf::new();
    for _ in from_iter {
        result.push("..");
    }
    for component in to_iter {
        result.push(component);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_external_path_rejects_parent() {
        assert!(validate_external_path("../foo").is_err());
        assert!(validate_external_path("foo/../bar").is_err());
    }

    #[test]
    fn validate_external_path_rejects_absolute() {
        assert!(validate_external_path("/etc").is_err());
    }

    #[test]
    fn validate_external_path_accepts_relative() {
        assert!(validate_external_path("napi/foo.node").is_ok());
        assert!(validate_external_path("wasm/").is_ok());
        assert!(validate_external_path("a/b/c").is_ok());
    }

    #[test]
    fn diff_paths_sibling_dirs() {
        assert_eq!(
            diff_paths(Path::new("/a/b/src"), Path::new("/a/b/out")),
            PathBuf::from("../src"),
        );
    }

    #[test]
    fn diff_paths_deeper_output() {
        assert_eq!(
            diff_paths(
                Path::new("/project/src"),
                Path::new("/project/.rust-curse/prelude"),
            ),
            PathBuf::from("../../src"),
        );
    }

    #[test]
    fn diff_paths_same_dir() {
        assert_eq!(
            diff_paths(Path::new("/a/b"), Path::new("/a/b")),
            PathBuf::from(""),
        );
    }
}
