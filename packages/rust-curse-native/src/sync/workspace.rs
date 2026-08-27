//! Populate compiler workspaces from Cargo projects on disk.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use cargo_metadata::{
    DependencyKind, MetadataCommand, NodeDep, Package, PackageId, Target, TargetKind,
};
use glob::Pattern;
use thiserror::Error;

use crate::compiler::{
    CrateApiDiscovery, CrateApiImportMode, CrateBuilderId, CrateGraphBuilder, Edition, FilePath,
    WorkspaceChange, WorkspaceChangeError, WorkspaceCrate, WorkspaceCrateDependency,
};

/// Options for [`WorkspaceChange::populate_from_fs`].
#[derive(Debug, Clone)]
pub struct PopulateWorkspaceOpts<'a> {
    /// Path to the manifest of the package that drives API discovery.
    pub manifest_path: &'a Path,
    /// Glob patterns for files loaded into the compiler.
    pub included: Vec<String>,
    /// Glob patterns for files and directories to skip.
    pub excluded: Vec<String>,
    /// Additional dependency package or crate names loaded with lazy discovery.
    /// Every resolved version with a matching name is included.
    pub include_dep_crates: Vec<String>,
    /// Dependency package or crate names omitted from the compiler workspace.
    /// Every resolved version with a matching name is excluded.
    pub exclude_dep_crates: Vec<String>,
    /// Whether standard-library crates should be loaded from the Rust sysroot.
    pub include_std: bool,
}

impl<'a> PopulateWorkspaceOpts<'a> {
    pub fn new(manifest_path: &'a Path) -> Self {
        Self {
            manifest_path,
            included: vec!["*.rs".to_owned()],
            excluded: vec![
                ".git".to_owned(),
                ".hg".to_owned(),
                ".svn".to_owned(),
                "node_modules".to_owned(),
                "target".to_owned(),
            ],
            include_dep_crates: Vec::new(),
            exclude_dep_crates: Vec::new(),
            include_std: true,
        }
    }
}

struct SourceFilters {
    included: Vec<WorkspacePattern>,
    excluded: Vec<WorkspacePattern>,
}

struct WorkspacePattern {
    glob: Pattern,
    basename_only: bool,
}

impl SourceFilters {
    fn new(opts: &PopulateWorkspaceOpts<'_>) -> Result<Self, PopulateWorkspaceError> {
        Ok(Self {
            included: compile_patterns(&opts.included)?,
            excluded: compile_patterns(&opts.excluded)?,
        })
    }

    fn includes(&self, relative_path: &Path) -> bool {
        self.included
            .iter()
            .any(|pattern| pattern.matches(relative_path))
    }

    fn excludes(&self, relative_path: &Path) -> bool {
        self.excluded
            .iter()
            .any(|pattern| pattern.matches(relative_path))
    }
}

impl WorkspacePattern {
    fn matches(&self, relative_path: &Path) -> bool {
        if self.basename_only {
            relative_path
                .file_name()
                .is_some_and(|name| self.glob.matches_path(Path::new(name)))
        } else {
            self.glob.matches_path(relative_path)
        }
    }
}

fn compile_patterns(patterns: &[String]) -> Result<Vec<WorkspacePattern>, PopulateWorkspaceError> {
    patterns
        .iter()
        .map(|pattern| {
            Ok(WorkspacePattern {
                basename_only: !pattern.contains(['/', '\\']),
                glob: Pattern::new(pattern).map_err(|source| {
                    PopulateWorkspaceError::InvalidPattern {
                        pattern: pattern.clone(),
                        source,
                    }
                })?,
            })
        })
        .collect()
}

impl WorkspaceChange {
    /// Loads all reachable Cargo packages, their Rust sources, and dependency
    /// edges into this workspace change.
    pub fn populate_from_fs(
        &mut self,
        opts: PopulateWorkspaceOpts<'_>,
    ) -> Result<(), WorkspaceChangeError> {
        self.try_populate_from_fs(opts)
            .map_err(WorkspaceChangeError::prepare)
    }

    fn try_populate_from_fs(
        &mut self,
        opts: PopulateWorkspaceOpts<'_>,
    ) -> Result<(), PopulateWorkspaceError> {
        let filters = SourceFilters::new(&opts)?;
        let included = opts
            .include_dep_crates
            .iter()
            .map(|name| normalize_crate_name(name))
            .collect::<HashSet<_>>();
        let excluded = opts
            .exclude_dep_crates
            .iter()
            .map(|name| normalize_crate_name(name))
            .collect::<HashSet<_>>();
        if let Some(name) = included.intersection(&excluded).next() {
            return Err(PopulateWorkspaceError::ConflictingCrateOverride { name: name.clone() });
        }

        let metadata = MetadataCommand::new()
            .manifest_path(opts.manifest_path)
            .exec()
            .map_err(|source| PopulateWorkspaceError::CargoMetadata {
                manifest_path: opts.manifest_path.to_path_buf(),
                source,
            })?;

        let root =
            metadata
                .root_package()
                .ok_or_else(|| PopulateWorkspaceError::MissingRootPackage {
                    manifest_path: opts.manifest_path.to_path_buf(),
                })?;

        let resolve = metadata
            .resolve
            .as_ref()
            .ok_or(PopulateWorkspaceError::MissingResolveGraph)?;

        let nodes = resolve
            .nodes
            .iter()
            .map(|node| (&node.id, node))
            .collect::<HashMap<_, _>>();
        let root_node =
            nodes
                .get(&root.id)
                .ok_or_else(|| PopulateWorkspaceError::MissingPackage {
                    package_id: root.id.repr.clone(),
                })?;
        let direct_dependencies = root_node
            .deps
            .iter()
            .filter(|dependency| is_normal(dependency))
            .map(|dependency| dependency.pkg.clone())
            .collect::<HashSet<_>>();

        let mut distances = HashMap::from([(root.id.clone(), 0usize)]);
        let mut pending = VecDeque::from([root.id.clone()]);

        while let Some(package_id) = pending.pop_front() {
            let distance = distances[&package_id];
            let Some(node) = nodes.get(&package_id) else {
                continue;
            };
            for dependency in node.deps.iter().filter(|dependency| is_normal(dependency)) {
                if distances.contains_key(&dependency.pkg) {
                    continue;
                }
                distances.insert(dependency.pkg.clone(), distance + 1);
                pending.push_back(dependency.pkg.clone());
            }
        }

        let packages = metadata
            .packages
            .iter()
            .map(|package| (&package.id, package))
            .collect::<HashMap<_, _>>();
        let workspace_members = metadata.workspace_members.iter().collect::<HashSet<_>>();

        let mut crate_graph = CrateGraphBuilder::default();
        let mut crate_ids = HashMap::<PackageId, CrateBuilderId>::new();
        let mut matched_includes = HashSet::new();
        let mut matched_excludes = HashSet::new();

        let mut reachable = distances.iter().collect::<Vec<_>>();
        reachable.sort_by_key(|(_, distance)| **distance);
        for (package_id, _) in reachable {
            let package =
                packages
                    .get(package_id)
                    .ok_or_else(|| PopulateWorkspaceError::MissingPackage {
                        package_id: package_id.repr.clone(),
                    })?;
            let Some(target) = library_target(package) else {
                continue;
            };

            let names = package_names(package, target);
            let is_main = package.id == root.id;
            let is_direct = direct_dependencies.contains(&package.id);
            let is_included = names.iter().any(|name| {
                let matches = included.contains(name);
                if matches {
                    matched_includes.insert(name.clone());
                }
                matches
            });
            let is_excluded = names.iter().any(|name| {
                let matches = excluded.contains(name);
                if matches {
                    matched_excludes.insert(name.clone());
                }
                matches
            });
            if is_main && is_excluded {
                return Err(PopulateWorkspaceError::ExcludedMainCrate {
                    name: package.name.to_string(),
                });
            }
            if (!is_main && is_excluded) || (!is_main && !is_direct && !is_included) {
                continue;
            }

            let is_local = workspace_members.contains(package_id);
            self.load_package_sources(package, is_local, &filters)?;
            let root_file = real_file_path(target.src_path.as_std_path())?;
            let (api_discovery, api_import_mode) = if is_main {
                (CrateApiDiscovery::Eager, CrateApiImportMode::Relative)
            } else if is_direct {
                (CrateApiDiscovery::Eager, CrateApiImportMode::Mod)
            } else {
                (CrateApiDiscovery::Lazy, CrateApiImportMode::Mod)
            };
            let crate_id = crate_graph.add_crate(WorkspaceCrate {
                name: target.name.clone(),
                edition: edition(target.edition.to_string().as_str())?,
                root_file,
                version: package.version.to_string(),
                is_local,
                is_sysroot: false,
                api_discovery,
                api_import_mode,
            });
            crate_ids.insert(package.id.clone(), crate_id);
        }

        for name in included.difference(&matched_includes) {
            if opts.include_std && is_sysroot_name(name) {
                continue;
            }
            return Err(PopulateWorkspaceError::UnknownCrateOverride {
                kind: "included",
                name: name.clone(),
            });
        }
        for name in excluded.difference(&matched_excludes) {
            if is_sysroot_name(name) {
                if opts.include_std {
                    return Err(PopulateWorkspaceError::ExcludedSysrootCrate {
                        name: name.clone(),
                    });
                }
                continue;
            }
            return Err(PopulateWorkspaceError::UnknownCrateOverride {
                kind: "excluded",
                name: name.clone(),
            });
        }

        let mut dependencies = HashSet::new();
        for package_id in distances.keys() {
            let Some(from_crate) = crate_ids.get(package_id).copied() else {
                continue;
            };
            let Some(node) = nodes.get(package_id) else {
                continue;
            };
            for dependency in node.deps.iter().filter(|dependency| is_normal(dependency)) {
                let Some(to_crate) = crate_ids.get(&dependency.pkg).copied() else {
                    continue;
                };
                let name = dependency.name.replace('-', "_");
                if dependencies.insert((from_crate, to_crate, name.clone())) {
                    crate_graph.add_dependency(WorkspaceCrateDependency {
                        name,
                        from_crate,
                        to_crate,
                    });
                }
            }
        }

        if opts.include_std {
            self.add_sysroot(&mut crate_graph, crate_ids.values().copied(), &filters)?;
        }
        self.set_crates(crate_graph);
        Ok(())
    }

    fn add_sysroot(
        &mut self,
        crate_graph: &mut CrateGraphBuilder,
        workspace_crates: impl Iterator<Item = CrateBuilderId>,
        filters: &SourceFilters,
    ) -> Result<(), PopulateWorkspaceError> {
        let rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
        let output = Command::new(&rustc)
            .args(["--print", "sysroot"])
            .output()
            .map_err(|source| PopulateWorkspaceError::RunRustc {
                rustc: PathBuf::from(&rustc),
                source,
            })?;
        if !output.status.success() {
            return Err(PopulateWorkspaceError::RustcFailed {
                rustc: PathBuf::from(&rustc),
                message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        let sysroot = PathBuf::from(String::from_utf8(output.stdout)?.trim());
        let library_dir = sysroot.join("lib/rustlib/src/rust/library");
        if !library_dir.join("std/src/lib.rs").is_file() {
            return Err(PopulateWorkspaceError::MissingRustSrc { sysroot });
        }

        let mut sysroot_crates = HashMap::new();
        for name in ["core", "alloc", "std", "proc_macro"] {
            let manifest_path = library_dir.join(name).join("Cargo.toml");
            let mut command = MetadataCommand::new();
            command.manifest_path(&manifest_path).no_deps();
            let metadata =
                command
                    .exec()
                    .map_err(|source| PopulateWorkspaceError::CargoMetadata {
                        manifest_path: manifest_path.clone(),
                        source,
                    })?;
            let package = metadata
                .packages
                .iter()
                .find(|package| package.name.as_str() == name)
                .ok_or_else(|| PopulateWorkspaceError::MissingRootPackage {
                    manifest_path: manifest_path.clone(),
                })?;
            let target = library_target(package).ok_or_else(|| {
                PopulateWorkspaceError::MissingLibraryTarget {
                    package: package.name.to_string(),
                }
            })?;

            self.load_package_sources(package, false, filters)?;
            let crate_id = crate_graph.add_crate(WorkspaceCrate {
                name: name.to_owned(),
                edition: edition(target.edition.to_string().as_str())?,
                root_file: real_file_path(target.src_path.as_std_path())?,
                version: package.version.to_string(),
                is_local: false,
                is_sysroot: true,
                api_discovery: CrateApiDiscovery::Lazy,
                api_import_mode: CrateApiImportMode::Mod,
            });
            sysroot_crates.insert(name, crate_id);
        }

        let core = sysroot_crates["core"];
        let alloc = sysroot_crates["alloc"];
        let std = sysroot_crates["std"];
        let proc_macro = sysroot_crates["proc_macro"];
        crate_graph.add_dependency(WorkspaceCrateDependency {
            name: "core".to_owned(),
            from_crate: alloc,
            to_crate: core,
        });
        for from_crate in [std, proc_macro] {
            crate_graph.add_dependency(WorkspaceCrateDependency {
                name: "core".to_owned(),
                from_crate,
                to_crate: core,
            });
            crate_graph.add_dependency(WorkspaceCrateDependency {
                name: "alloc".to_owned(),
                from_crate,
                to_crate: alloc,
            });
        }
        crate_graph.add_dependency(WorkspaceCrateDependency {
            name: "std".to_owned(),
            from_crate: proc_macro,
            to_crate: std,
        });
        for from_crate in workspace_crates {
            crate_graph.add_dependency(WorkspaceCrateDependency {
                name: "std".to_owned(),
                from_crate,
                to_crate: std,
            });
        }

        Ok(())
    }

    fn load_package_sources(
        &mut self,
        package: &Package,
        is_local: bool,
        filters: &SourceFilters,
    ) -> Result<(), PopulateWorkspaceError> {
        let package_dir = package
            .manifest_path
            .as_std_path()
            .parent()
            .ok_or_else(|| PopulateWorkspaceError::ManifestWithoutParent {
                manifest_path: package.manifest_path.as_std_path().to_path_buf(),
            })?;
        let mut pending = vec![package_dir.to_path_buf()];

        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(&directory).map_err(|source| {
                PopulateWorkspaceError::ReadDirectory {
                    path: directory.clone(),
                    source,
                }
            })? {
                let entry = entry.map_err(|source| PopulateWorkspaceError::ReadDirectory {
                    path: directory.clone(),
                    source,
                })?;
                let path = entry.path();
                let file_type =
                    entry
                        .file_type()
                        .map_err(|source| PopulateWorkspaceError::ReadFileType {
                            path: path.clone(),
                            source,
                        })?;
                let relative_path = path.strip_prefix(package_dir).map_err(|_| {
                    PopulateWorkspaceError::SourceOutsidePackage {
                        path: path.clone(),
                        package_dir: package_dir.to_path_buf(),
                    }
                })?;

                if file_type.is_dir() {
                    if !filters.excludes(relative_path) {
                        pending.push(path);
                    }
                } else if file_type.is_file()
                    && !filters.excludes(relative_path)
                    && filters.includes(relative_path)
                {
                    let contents = fs::read_to_string(&path).map_err(|source| {
                        PopulateWorkspaceError::ReadSource {
                            path: path.clone(),
                            source,
                        }
                    })?;
                    let path = real_file_path(&path)?;
                    if is_local {
                        self.set_local_file(path, Some(contents));
                    } else {
                        self.set_library_file(path, Some(contents));
                    }
                }
            }
        }
        Ok(())
    }
}

fn library_target(package: &Package) -> Option<&Target> {
    package.targets.iter().find(|target| {
        target.kind.iter().any(|kind| {
            matches!(
                kind,
                TargetKind::CDyLib
                    | TargetKind::DyLib
                    | TargetKind::Lib
                    | TargetKind::ProcMacro
                    | TargetKind::RLib
                    | TargetKind::StaticLib
            )
        })
    })
}

fn package_names(package: &Package, target: &Target) -> [String; 2] {
    [
        normalize_crate_name(package.name.as_str()),
        normalize_crate_name(&target.name),
    ]
}

fn normalize_crate_name(name: &str) -> String {
    name.replace('-', "_")
}

fn is_sysroot_name(name: &str) -> bool {
    matches!(name, "alloc" | "core" | "proc_macro" | "std")
}

fn is_normal(dependency: &NodeDep) -> bool {
    dependency
        .dep_kinds
        .iter()
        .any(|kind| kind.kind == DependencyKind::Normal)
}

fn real_file_path(path: &Path) -> Result<FilePath, PopulateWorkspaceError> {
    let path = path
        .canonicalize()
        .map_err(|source| PopulateWorkspaceError::CanonicalizePath {
            path: path.to_path_buf(),
            source,
        })?;
    let path = path
        .to_str()
        .ok_or_else(|| PopulateWorkspaceError::NonUtf8Path { path: path.clone() })?;
    FilePath::new_real(path).map_err(|source| PopulateWorkspaceError::FilePath { source })
}

fn edition(value: &str) -> Result<Edition, PopulateWorkspaceError> {
    match value {
        "2015" => Ok(Edition::E2015),
        "2018" => Ok(Edition::E2018),
        "2021" => Ok(Edition::E2021),
        "2024" => Ok(Edition::E2024),
        value => Err(PopulateWorkspaceError::UnsupportedEdition {
            edition: value.to_owned(),
        }),
    }
}

#[derive(Debug, Error)]
enum PopulateWorkspaceError {
    #[error("failed to read Cargo metadata for {}", manifest_path.display())]
    CargoMetadata {
        manifest_path: PathBuf,
        #[source]
        source: cargo_metadata::Error,
    },
    #[error("Cargo manifest {} does not identify a root package", manifest_path.display())]
    MissingRootPackage { manifest_path: PathBuf },
    #[error("Cargo metadata did not contain a dependency graph")]
    MissingResolveGraph,
    #[error("failed to run {} --print sysroot", rustc.display())]
    RunRustc {
        rustc: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{} --print sysroot failed: {message}", rustc.display())]
    RustcFailed { rustc: PathBuf, message: String },
    #[error(
        "Rust source is not installed for sysroot {}; install the rust-src component",
        sysroot.display()
    )]
    MissingRustSrc { sysroot: PathBuf },
    #[error("package `{package}` does not have a library target")]
    MissingLibraryTarget { package: String },
    #[error("Cargo dependency graph references missing package {package_id}")]
    MissingPackage { package_id: String },
    #[error("invalid workspace path pattern `{pattern}`")]
    InvalidPattern {
        pattern: String,
        #[source]
        source: glob::PatternError,
    },
    #[error("crate `{name}` cannot be both included and excluded")]
    ConflictingCrateOverride { name: String },
    #[error("the main crate `{name}` cannot be excluded")]
    ExcludedMainCrate { name: String },
    #[error("the sysroot crate `{name}` is always included and cannot be excluded")]
    ExcludedSysrootCrate { name: String },
    #[error("{kind} dependency crate `{name}` was not found")]
    UnknownCrateOverride { kind: &'static str, name: String },
    #[error("manifest path {} has no parent", manifest_path.display())]
    ManifestWithoutParent { manifest_path: PathBuf },
    #[error("failed to read directory {}", path.display())]
    ReadDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read file type for {}", path.display())]
    ReadFileType {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read Rust source {}", path.display())]
    ReadSource {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "source path {} is outside package {}",
        path.display(),
        package_dir.display()
    )]
    SourceOutsidePackage { path: PathBuf, package_dir: PathBuf },
    #[error("failed to canonicalize {}", path.display())]
    CanonicalizePath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("path is not valid UTF-8: {}", path.display())]
    NonUtf8Path { path: PathBuf },
    #[error("unsupported Rust edition {edition}")]
    UnsupportedEdition { edition: String },
    #[error("command output is not valid UTF-8")]
    InvalidCommandOutput(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    FilePath {
        #[from]
        source: crate::compiler::FilePathError,
    },
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::compiler::{CompileShimOpts, Compiler, Platform};

    #[test]
    fn includes_overridden_transitive_dependency() {
        let workspace = tempdir().unwrap();
        fs::write(
            workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"root\", \"direct\", \"transitive\"]\nresolver = \"3\"\n",
        )
        .unwrap();
        write_package(
            workspace.path(),
            "transitive",
            "",
            "pub struct Transitive;\n",
        );
        write_package(
            workspace.path(),
            "direct",
            "transitive = { path = \"../transitive\" }\n",
            "pub struct Direct(pub transitive::Transitive);\n",
        );
        write_package(
            workspace.path(),
            "root",
            "direct = { path = \"../direct\" }\n",
            "pub fn root(value: direct::Direct) { let _ = value; }\n",
        );

        let manifest_path = workspace.path().join("root/Cargo.toml");
        let mut compiler = Compiler::new();
        compiler
            .change_workspace(|change| {
                let mut opts = PopulateWorkspaceOpts::new(&manifest_path);
                opts.include_dep_crates.push("transitive".to_owned());
                change.populate_from_fs(opts)
            })
            .unwrap();

        let output = compiler.compile_shim(CompileShimOpts {
            platform: Platform::Node,
        });
        assert!(output.shim.contains("direct"), "{}", output.shim);
        assert!(output.shim.contains("transitive"), "{}", output.shim);
    }

    #[test]
    fn workspace_options_use_rust_source_defaults() {
        let opts = PopulateWorkspaceOpts::new(Path::new("Cargo.toml"));
        let filters = SourceFilters::new(&opts).unwrap();

        assert!(filters.includes(Path::new("src/lib.rs")));
        assert!(!filters.includes(Path::new("Cargo.toml")));
        assert!(filters.excludes(Path::new(".git")));
        assert!(filters.excludes(Path::new("nested/target")));
        assert!(opts.include_dep_crates.is_empty());
        assert!(opts.exclude_dep_crates.is_empty());
        assert!(opts.include_std);
    }

    #[test]
    fn workspace_options_support_path_globs() {
        let mut opts = PopulateWorkspaceOpts::new(Path::new("Cargo.toml"));
        opts.included = vec!["src/**/*.rs".to_owned()];
        opts.excluded = vec!["src/generated/**".to_owned()];
        let filters = SourceFilters::new(&opts).unwrap();

        assert!(filters.includes(Path::new("src/nested/lib.rs")));
        assert!(!filters.includes(Path::new("tests/integration.rs")));
        assert!(filters.excludes(Path::new("src/generated/bindings.rs")));
    }

    fn write_package(root: &Path, name: &str, dependencies: &str, source: &str) {
        let package = root.join(name);
        fs::create_dir_all(package.join("src")).unwrap();
        fs::write(
            package.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
                 [dependencies]\n{dependencies}"
            ),
        )
        .unwrap();
        fs::write(package.join("src/lib.rs"), source).unwrap();
    }
}
