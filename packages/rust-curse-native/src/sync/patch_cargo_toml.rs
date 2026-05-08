//! Utilities for patching `Cargo.toml` file.

use std::{
    collections::{BTreeMap, HashSet},
    fmt, fs, io,
    path::Path,
};

use toml_edit::{Array, DocumentMut, InlineTable, Item, Table};

/// Error returned when reading, parsing, or writing a `Cargo.toml`.
#[derive(Debug, thiserror::Error)]
pub enum PatchCargoTomlError {
    #[error("failed to parse Cargo.toml: {0}")]
    Parse(#[from] toml_edit::TomlError),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

/// A patch to be applied to a Cargo.toml file.
/// # Example
///
/// ```no_run
/// // no_run: remove once napi feature is removed
/// use rust_curse::sync::patch_cargo_toml::{patch_cargo_toml, CargoTomlPatch, Dep};
///
/// let patch = CargoTomlPatch::new()
///     .crate_type("cdylib")
///     .dep(Dep::new("anyhow", "1"));
///
/// let original = "[package]\nname = \"x\"\nversion = \"0.1\"\n";
/// let patched = patch_cargo_toml(original, &patch).unwrap();
/// assert!(patched.contains("anyhow"));
/// ```
#[derive(Default, Debug)]
pub struct CargoTomlPatch {
    pub deps: Option<HashSet<Dep>>,
    pub build_deps: Option<HashSet<Dep>>,
    pub lib_path: Option<String>,
    pub build_path: Option<String>,
    pub crate_type: Option<HashSet<String>>,
    pub features: Option<HashSet<String>>,
}

impl CargoTomlPatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Specifies a dependency to be added to the Cargo.toml.
    pub fn dep(mut self, dep: Dep) -> Self {
        self.deps.get_or_insert_with(HashSet::new).insert(dep);
        self
    }

    /// Specifies a build dependency to be added to the Cargo.toml.
    pub fn build_dep(mut self, dep: Dep) -> Self {
        self.build_deps.get_or_insert_with(HashSet::new).insert(dep);
        self
    }

    /// Specifies the crate type to be added to the Cargo.toml, e.g `cdylib`
    pub fn crate_type(mut self, crate_type: impl Into<String>) -> Self {
        self.crate_type
            .get_or_insert_with(HashSet::new)
            .insert(crate_type.into());
        self
    }

    /// Specifies the lib path to be added to the Cargo.toml, e.g `lib`
    pub fn lib_path(mut self, lib_path: impl Into<String>) -> Self {
        self.lib_path = Some(lib_path.into());
        self
    }

    /// Specifies the build path to be added to the Cargo.toml, e.g `build`
    pub fn build_path(mut self, build_path: impl Into<String>) -> Self {
        self.build_path = Some(build_path.into());
        self
    }

    /// Declares an empty feature in `[features]`, e.g. `client = []`.
    pub fn empty_feature(mut self, name: impl Into<String>) -> Self {
        self.features
            .get_or_insert_with(HashSet::new)
            .insert(name.into());
        self
    }
}

/// A cargo dependency to be added to the patch.
#[derive(Default, Debug, Hash, Eq, PartialEq, Clone)]
pub struct Dep {
    /// The name of the dependency to add, e.g `serde`.
    pub name: String,
    /// The version of the dependency to add, e.g `1.0.0`.
    pub version: String,
    /// Features to enable for this dependency.
    pub features: Option<Vec<String>>,
    /// Features that must be enabled for this dependency to be used. When
    /// set, the dep is written as `optional = true` and `dep:<name>` is
    /// appended to the listed entries in `[features]`.
    pub require_features: Option<Vec<String>>,
}

impl Dep {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            ..Default::default()
        }
    }

    /// Specifies the feature to enable for this dependency.
    pub fn feature(mut self, feature: impl Into<String>) -> Self {
        let mut features = self.features.unwrap_or_default();
        features.push(feature.into());
        self.features = Some(features);
        self
    }

    /// Specifies the feature that must be enabled for this dependency to be
    /// used.
    pub fn requires_feature(mut self, require_feature: impl Into<String>) -> Self {
        let mut require_features = self.require_features.unwrap_or_default();
        require_features.push(require_feature.into());
        self.require_features = Some(require_features);
        self
    }
}

impl fmt::Display for Dep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.name, self.version)?;
        if let Some(features) = self.features.as_ref().filter(|v| !v.is_empty()) {
            write!(f, " features={features:?}")?;
        }
        if let Some(rf) = self.require_features.as_ref().filter(|v| !v.is_empty()) {
            write!(f, " require_features={rf:?}")?;
        }
        Ok(())
    }
}

impl fmt::Display for CargoTomlPatch {
    /// Renders the patch in a deterministic, human-readable form. The
    /// `HashSet`-backed fields are sorted before printing so the output is
    /// stable across runs.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(lp) = &self.lib_path {
            writeln!(f, "lib_path = {lp:?}")?;
        }
        if let Some(bp) = &self.build_path {
            writeln!(f, "build_path = {bp:?}")?;
        }
        if let Some(ct) = &self.crate_type {
            let mut sorted: Vec<&String> = ct.iter().collect();
            sorted.sort();
            writeln!(f, "crate_type = {sorted:?}")?;
        }
        if let Some(deps) = &self.deps {
            writeln!(f, "deps:")?;
            for dep in sorted_deps(deps) {
                writeln!(f, "  - {dep}")?;
            }
        }
        if let Some(deps) = &self.build_deps {
            writeln!(f, "build_deps:")?;
            for dep in sorted_deps(deps) {
                writeln!(f, "  - {dep}")?;
            }
        }
        if let Some(features) = &self.features {
            let mut sorted: Vec<&String> = features.iter().collect();
            sorted.sort();
            writeln!(f, "features = {sorted:?}")?;
        }
        Ok(())
    }
}

fn sorted_deps(deps: &HashSet<Dep>) -> Vec<&Dep> {
    let mut v: Vec<&Dep> = deps.iter().collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

/// Reads `path`, applies `patch`, writes the result back to `path`.
///
/// # Example
///
/// ```no_run
/// // no_run: remove once napi feature is removed
/// use rust_curse::sync::patch_cargo_toml::{patch_cargo_toml_file, CargoTomlPatch, Dep};
/// use std::path::Path;
///
/// let patch = CargoTomlPatch::new().dep(Dep::new("serde", "1"));
/// patch_cargo_toml_file(Path::new("Cargo.toml"), &patch).unwrap();
/// ```
pub fn patch_cargo_toml_file(
    path: &Path,
    patch: &CargoTomlPatch,
) -> Result<(), PatchCargoTomlError> {
    let content = fs::read_to_string(path)?;
    let patched = patch_cargo_toml(&content, patch)?;
    fs::write(path, patched)?;
    Ok(())
}

/// Applies `patch` to the given Cargo.toml `content` and returns the new
/// content. Existing formatting and unrelated keys are preserved.
///
/// # Example
///
/// ```no_run
/// // no_run: remove once napi feature is removed
/// use rust_curse::sync::patch_cargo_toml::{patch_cargo_toml, CargoTomlPatch};
///
/// let patch = CargoTomlPatch::new().lib_path("src/lib.rs");
/// let original = "[package]\nname = \"x\"\nversion = \"0.1\"\n";
/// let patched = patch_cargo_toml(original, &patch).unwrap();
/// assert!(patched.contains("path = \"src/lib.rs\""));
/// ```
pub fn patch_cargo_toml(
    content: &str,
    patch: &CargoTomlPatch,
) -> Result<String, PatchCargoTomlError> {
    let mut doc: DocumentMut = content.parse()?;

    if let Some(lib_path) = &patch.lib_path {
        lib_table(&mut doc).insert("path", toml_edit::value(lib_path.as_str()));
    }

    if let Some(build_path) = &patch.build_path {
        let pkg = doc["package"].or_insert(Item::Table(Table::new()));
        let pkg = pkg.as_table_mut().expect("[package] should be a table");
        pkg.insert("build", toml_edit::value(build_path.as_str()));
    }

    if let Some(crate_types) = &patch.crate_type {
        let mut merged = read_string_array(
            doc.get("lib")
                .and_then(Item::as_table)
                .and_then(|t| t.get("crate-type")),
        );
        merged.extend(crate_types.iter().cloned());
        if merged.len() == 1 && merged.contains("rlib") {
            if let Some(table) = doc.get_mut("lib").and_then(Item::as_table_mut) {
                table.remove("crate-type");
                if table.is_empty() {
                    doc.as_table_mut().remove("lib");
                }
            }
        } else {
            let table = lib_table(&mut doc);
            table.insert("crate-type", Item::Value(string_array(&merged).into()));
        }
    }

    if let Some(deps) = &patch.deps {
        upsert_deps(&mut doc, "dependencies", deps);
    }
    if let Some(build_deps) = &patch.build_deps {
        upsert_deps(&mut doc, "build-dependencies", build_deps);
    }

    let feature_additions = collect_feature_additions(patch);
    if !feature_additions.is_empty() {
        let features = doc["features"].or_insert(Item::Table(Table::new()));
        let table = features
            .as_table_mut()
            .expect("[features] should be a table");
        for (feat, dep_refs) in feature_additions {
            let mut merged = read_string_array(table.get(&feat));
            merged.extend(dep_refs);
            table.insert(&feat, Item::Value(string_array(&merged).into()));
        }
    }

    Ok(doc.to_string())
}

fn lib_table(doc: &mut DocumentMut) -> &mut Table {
    let lib = doc["lib"].or_insert(Item::Table(Table::new()));
    lib.as_table_mut().expect("[lib] should be a table")
}

fn read_string_array(item: Option<&Item>) -> HashSet<String> {
    item.and_then(Item::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn string_array(values: &HashSet<String>) -> Array {
    let mut sorted: Vec<&String> = values.iter().collect();
    sorted.sort();
    let mut arr = Array::new();
    for s in sorted {
        arr.push(s.as_str());
    }
    arr
}

fn collect_feature_additions(patch: &CargoTomlPatch) -> BTreeMap<String, HashSet<String>> {
    let mut out: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    let all = patch
        .deps
        .iter()
        .flatten()
        .chain(patch.build_deps.iter().flatten());
    for dep in all {
        let Some(rf) = &dep.require_features else {
            continue;
        };
        for feat in rf {
            out.entry(feat.clone())
                .or_default()
                .insert(format!("dep:{}", dep.name));
        }
    }
    if let Some(features) = &patch.features {
        for feat in features {
            out.entry(feat.clone()).or_default();
        }
    }
    out
}

fn upsert_deps(doc: &mut DocumentMut, table_key: &str, deps: &HashSet<Dep>) {
    let item = doc[table_key].or_insert(Item::Table(Table::new()));
    let table = item
        .as_table_mut()
        .expect("dependencies-like table should be a table");

    let mut sorted: Vec<&Dep> = deps.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    for dep in sorted {
        if is_workspace_dep(table.get(&dep.name)) {
            continue;
        }

        let optional = dep.require_features.as_ref().is_some_and(|v| !v.is_empty());

        let mut t = InlineTable::new();
        t.insert("version", dep.version.clone().into());
        if let Some(features) = &dep.features {
            if !features.is_empty() {
                let mut arr = Array::new();
                for f in features {
                    arr.push(f.as_str());
                }
                t.insert("features", arr.into());
            }
        }
        if optional {
            t.insert("optional", true.into());
        }
        table.insert(&dep.name, Item::Value(t.into()));
    }
}

/// True when the table cell is a `{ workspace = true }` (inline or full-table)
/// dependency. Such entries are inherited from the workspace and must not be
/// overwritten by a patch.
fn is_workspace_dep(item: Option<&Item>) -> bool {
    let Some(item) = item else { return false };
    if let Some(t) = item.as_inline_table() {
        if t.get("workspace").and_then(|v| v.as_bool()) == Some(true) {
            return true;
        }
    }
    if let Some(t) = item.as_table() {
        if t.get("workspace")
            .and_then(|v| v.as_value())
            .and_then(|v| v.as_bool())
            == Some(true)
        {
            return true;
        }
    }
    false
}
