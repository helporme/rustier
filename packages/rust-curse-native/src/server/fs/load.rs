//! Load server filesystem state from a Cargo project.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use cargo_metadata::{
    DependencyKind, MetadataCommand, Node, NodeDep, Package, PackageId, Target, TargetKind,
};
use glob::Pattern;
use miette::Diagnostic;
use thiserror::Error;

use crate::compiler::{
    CrateApiDiscovery, CrateApiImportMode, CrateBuilderId, CrateGraphBuilder, Edition, FilePath,
    WorkspaceChange, WorkspaceCrate, WorkspaceCrateDependency, WorkspaceCrateKind,
};

use super::ServerFs;

/// Options for [`ServerFs::load`].
#[derive(Debug, Clone)]
pub struct ServerFsLoadOpts<'a> {
    /// Path to the manifest of the package that drives API discovery.
    pub manifest_path: &'a Path,
    /// Glob patterns for files loaded into the compiler.
    pub included: Vec<Pattern>,
    /// Glob patterns for files and directories to skip.
    pub excluded: Vec<Pattern>,
    /// Additional dependency package or crate names loaded with lazy discovery.
    /// Every resolved version with a matching name is included.
    pub include_dep_crates: HashSet<String>,
    /// Dependency package or crate names omitted from the compiler.
    /// Every resolved version with a matching name is excluded.
    pub exclude_dep_crates: HashSet<String>,
    /// Whether standard-library crates should be loaded from the Rust sysroot.
    pub include_std: bool,
}

impl<'a> ServerFsLoadOpts<'a> {
    /// Creates load options with Rust source and common build-directory
    /// filters.
    pub fn new(manifest_path: &'a Path) -> Self {
        Self {
            manifest_path,
            included: vec![Pattern::new("*.rs").unwrap()],
            excluded: vec![
                Pattern::new(".git").unwrap(),
                Pattern::new(".hg").unwrap(),
                Pattern::new(".svn").unwrap(),
                Pattern::new("node_modules").unwrap(),
                Pattern::new("target").unwrap(),
            ],
            include_dep_crates: HashSet::new(),
            exclude_dep_crates: HashSet::new(),
            include_std: true,
        }
    }
}

impl ServerFs {
    /// Loads filesystem state and compiler inputs from a Cargo project.
    pub fn load(opts: ServerFsLoadOpts<'_>) -> Result<Self, FsLoadError> {
        let mut server_fs = Self {
            pending_change: WorkspaceChange::new(),
            active_files: Vec::new(),
        };
        server_fs.load_project(opts)?;
        server_fs.active_files.sort();
        server_fs.active_files.dedup();
        Ok(server_fs)
    }

    fn load_project(&mut self, opts: ServerFsLoadOpts<'_>) -> Result<(), FsLoadError> {
        let filters = SourceFilters {
            included: opts.included.clone(),
            excluded: opts.excluded.clone(),
        };
        let overrides = CrateOverrides::new(&opts.include_dep_crates, &opts.exclude_dep_crates)?;
        let metadata = MetadataCommand::new()
            .manifest_path(opts.manifest_path)
            .exec()
            .map_err(|source| FsLoadError::CargoMetadata {
                manifest_path: opts.manifest_path.to_path_buf(),
                source,
            })?;

        let project = CargoProject::new(&metadata, opts.manifest_path)?;
        self.track_cargo_files(&metadata, &project.packages, &project.members)?;
        let LoadedCrates {
            mut graph,
            ids,
            matched_includes,
            matched_excludes,
        } = self.load_crates(&project, &filters, &overrides.included, &overrides.excluded)?;
        overrides.validate(&matched_includes, &matched_excludes, opts.include_std)?;

        add_dependencies(&mut graph, project.distances.keys(), &project.nodes, &ids);

        if opts.include_std {
            self.add_sysroot(&mut graph, ids.values().copied(), &filters)?;
        }
        self.pending_change.set_crates(graph);
        Ok(())
    }

    fn load_crates(
        &mut self,
        project: &CargoProject<'_>,
        filters: &SourceFilters,
        included: &HashSet<String>,
        excluded: &HashSet<String>,
    ) -> Result<LoadedCrates, FsLoadError> {
        let mut loaded = LoadedCrates::default();
        let mut reachable = project.distances.iter().collect::<Vec<_>>();
        reachable.sort_by_key(|(_, distance)| **distance);

        for (package_id, _) in reachable {
            let package =
                project
                    .packages
                    .get(package_id)
                    .ok_or_else(|| FsLoadError::MissingPackage {
                        package_id: package_id.repr.clone(),
                    })?;
            let Some(target) = library_target(package) else {
                continue;
            };
            let names = [
                normalize_crate_name(package.name.as_str()),
                normalize_crate_name(&target.name),
            ];
            let is_main = package.id == project.root.id;
            let is_direct = project.direct_dependencies.contains(&package.id);
            let is_included = names.iter().any(|name| {
                let matches = included.contains(name);
                if matches {
                    loaded.matched_includes.insert(name.clone());
                }
                matches
            });
            let is_excluded = names.iter().any(|name| {
                let matches = excluded.contains(name);
                if matches {
                    loaded.matched_excludes.insert(name.clone());
                }
                matches
            });
            if is_main && is_excluded {
                return Err(FsLoadError::ExcludedMainCrate {
                    name: package.name.to_string(),
                });
            }
            if (!is_main && is_excluded) || (!is_main && !is_direct && !is_included) {
                continue;
            }

            let is_local = project.members.contains(package_id);
            self.load_package_sources(package, is_local, filters)?;
            let (api_discovery, api_import_mode) = if is_main {
                (CrateApiDiscovery::Eager, CrateApiImportMode::Relative)
            } else if is_direct {
                (CrateApiDiscovery::Eager, CrateApiImportMode::Mod)
            } else {
                (CrateApiDiscovery::Lazy, CrateApiImportMode::Mod)
            };
            let crate_id = loaded.graph.add_crate(WorkspaceCrate {
                name: target.name.clone(),
                edition: edition(target.edition.to_string().as_str())?,
                root_file: real_file_path(target.src_path.as_std_path())?,
                version: package.version.to_string(),
                kind: if is_local {
                    WorkspaceCrateKind::Local
                } else {
                    WorkspaceCrateKind::Dependency
                },
                api_discovery,
                api_import_mode,
            });
            loaded.ids.insert(package.id.clone(), crate_id);
        }
        Ok(loaded)
    }

    fn track_cargo_files(
        &mut self,
        metadata: &cargo_metadata::Metadata,
        packages: &HashMap<&PackageId, &Package>,
        project_members: &HashSet<&PackageId>,
    ) -> Result<(), FsLoadError> {
        for package_id in project_members {
            if let Some(package) = packages.get(package_id) {
                self.active_files.push(path_string(real_file_path(
                    package.manifest_path.as_std_path(),
                )?));
            }
        }
        let lockfile = metadata.workspace_root.as_std_path().join("Cargo.lock");
        if lockfile.is_file() {
            self.active_files
                .push(path_string(real_file_path(&lockfile)?));
        }
        Ok(())
    }

    fn add_sysroot(
        &mut self,
        crate_graph: &mut CrateGraphBuilder,
        project_crates: impl Iterator<Item = CrateBuilderId>,
        filters: &SourceFilters,
    ) -> Result<(), FsLoadError> {
        let library_dir = discover_sysroot_library()?;
        let mut sysroot_crates = HashMap::new();
        for name in ["core", "alloc", "std", "proc_macro"] {
            let crate_id = self.load_sysroot_crate(crate_graph, &library_dir, name, filters)?;
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
        for from_crate in project_crates {
            crate_graph.add_dependency(WorkspaceCrateDependency {
                name: "std".to_owned(),
                from_crate,
                to_crate: std,
            });
        }

        Ok(())
    }

    fn load_sysroot_crate(
        &mut self,
        crate_graph: &mut CrateGraphBuilder,
        library_dir: &Path,
        name: &str,
        filters: &SourceFilters,
    ) -> Result<CrateBuilderId, FsLoadError> {
        let manifest_path = library_dir.join(name).join("Cargo.toml");
        let mut command = MetadataCommand::new();
        command.manifest_path(&manifest_path).no_deps();
        let metadata = command
            .exec()
            .map_err(|source| FsLoadError::CargoMetadata {
                manifest_path: manifest_path.clone(),
                source,
            })?;
        let package = metadata
            .packages
            .iter()
            .find(|package| package.name.as_str() == name)
            .ok_or_else(|| FsLoadError::MissingRootPackage {
                manifest_path: manifest_path.clone(),
            })?;
        let target = library_target(package).ok_or_else(|| FsLoadError::MissingLibraryTarget {
            package: package.name.to_string(),
        })?;

        self.load_package_sources(package, false, filters)?;
        Ok(crate_graph.add_crate(WorkspaceCrate {
            name: name.to_owned(),
            edition: edition(target.edition.to_string().as_str())?,
            root_file: real_file_path(target.src_path.as_std_path())?,
            version: package.version.to_string(),
            kind: WorkspaceCrateKind::Lang,
            api_discovery: CrateApiDiscovery::Lazy,
            api_import_mode: CrateApiImportMode::Mod,
        }))
    }

    fn load_package_sources(
        &mut self,
        package: &Package,
        is_local: bool,
        filters: &SourceFilters,
    ) -> Result<(), FsLoadError> {
        let package_dir = package
            .manifest_path
            .as_std_path()
            .parent()
            .ok_or_else(|| FsLoadError::ManifestWithoutParent {
                manifest_path: package.manifest_path.as_std_path().to_path_buf(),
            })?;
        let mut pending = vec![package_dir.to_path_buf()];

        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(&directory).map_err(|source| FsLoadError::ReadDirectory {
                path: directory.clone(),
                source,
            })? {
                let entry = entry.map_err(|source| FsLoadError::ReadDirectory {
                    path: directory.clone(),
                    source,
                })?;
                let path = entry.path();
                let file_type = entry
                    .file_type()
                    .map_err(|source| FsLoadError::ReadFileType {
                        path: path.clone(),
                        source,
                    })?;
                let relative_path = path.strip_prefix(package_dir).map_err(|_| {
                    FsLoadError::SourceOutsidePackage {
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
                    let contents =
                        fs::read_to_string(&path).map_err(|source| FsLoadError::ReadSource {
                            path: path.clone(),
                            source,
                        })?;
                    let path = real_file_path(&path)?;
                    if is_local {
                        self.active_files
                            .push(path.as_path().to_string_lossy().into_owned());
                        self.pending_change.set_local_file(path, Some(contents));
                    } else {
                        self.pending_change.set_library_file(path, Some(contents));
                    }
                }
            }
        }
        Ok(())
    }
}

struct CrateOverrides {
    included: HashSet<String>,
    excluded: HashSet<String>,
}

impl CrateOverrides {
    fn new(included: &HashSet<String>, excluded: &HashSet<String>) -> Result<Self, FsLoadError> {
        let included = included
            .iter()
            .map(|name| normalize_crate_name(name))
            .collect::<HashSet<_>>();
        let excluded = excluded
            .iter()
            .map(|name| normalize_crate_name(name))
            .collect::<HashSet<_>>();
        if let Some(name) = included.intersection(&excluded).next() {
            return Err(FsLoadError::ConflictingCrateOverride { name: name.clone() });
        }
        Ok(Self { included, excluded })
    }

    fn validate(
        &self,
        matched_includes: &HashSet<String>,
        matched_excludes: &HashSet<String>,
        include_std: bool,
    ) -> Result<(), FsLoadError> {
        for name in self.included.difference(matched_includes) {
            if include_std && is_sysroot_name(name) {
                continue;
            }
            return Err(FsLoadError::UnknownCrateOverride {
                kind: "included",
                name: name.clone(),
            });
        }
        for name in self.excluded.difference(matched_excludes) {
            if is_sysroot_name(name) {
                if include_std {
                    return Err(FsLoadError::ExcludedSysrootCrate { name: name.clone() });
                }
                continue;
            }
            return Err(FsLoadError::UnknownCrateOverride {
                kind: "excluded",
                name: name.clone(),
            });
        }
        Ok(())
    }
}

struct CargoProject<'a> {
    root: &'a Package,
    nodes: HashMap<&'a PackageId, &'a Node>,
    packages: HashMap<&'a PackageId, &'a Package>,
    direct_dependencies: HashSet<PackageId>,
    distances: HashMap<PackageId, usize>,
    members: HashSet<&'a PackageId>,
}

impl<'a> CargoProject<'a> {
    fn new(
        metadata: &'a cargo_metadata::Metadata,
        manifest_path: &Path,
    ) -> Result<Self, FsLoadError> {
        let root = metadata
            .root_package()
            .ok_or_else(|| FsLoadError::MissingRootPackage {
                manifest_path: manifest_path.to_path_buf(),
            })?;
        let resolve = metadata
            .resolve
            .as_ref()
            .ok_or(FsLoadError::MissingResolveGraph)?;
        let nodes = resolve
            .nodes
            .iter()
            .map(|node| (&node.id, node))
            .collect::<HashMap<_, _>>();
        let root_node = nodes
            .get(&root.id)
            .ok_or_else(|| FsLoadError::MissingPackage {
                package_id: root.id.repr.clone(),
            })?;
        let direct_dependencies = root_node
            .deps
            .iter()
            .filter(|dependency| is_normal(dependency))
            .map(|dependency| dependency.pkg.clone())
            .collect();
        let distances = dependency_distances(&root.id, &nodes);
        let packages = metadata
            .packages
            .iter()
            .map(|package| (&package.id, package))
            .collect();
        let members = metadata.workspace_members.iter().collect();

        Ok(Self {
            root,
            nodes,
            packages,
            direct_dependencies,
            distances,
            members,
        })
    }
}

#[derive(Default)]
struct LoadedCrates {
    graph: CrateGraphBuilder,
    ids: HashMap<PackageId, CrateBuilderId>,
    matched_includes: HashSet<String>,
    matched_excludes: HashSet<String>,
}

struct SourceFilters {
    included: Vec<Pattern>,
    excluded: Vec<Pattern>,
}

impl SourceFilters {
    fn includes(&self, relative_path: &Path) -> bool {
        self.included
            .iter()
            .any(|pattern| matches_pattern(pattern, relative_path))
    }

    fn excludes(&self, relative_path: &Path) -> bool {
        self.excluded
            .iter()
            .any(|pattern| matches_pattern(pattern, relative_path))
    }
}

fn matches_pattern(pattern: &Pattern, path: &Path) -> bool {
    pattern.matches_path(path)
        || path
            .file_name()
            .is_some_and(|file_name| pattern.matches_path(Path::new(file_name)))
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

fn dependency_distances(
    root: &PackageId,
    nodes: &HashMap<&PackageId, &Node>,
) -> HashMap<PackageId, usize> {
    let mut distances = HashMap::from([(root.clone(), 0)]);
    let mut pending = VecDeque::from([root.clone()]);
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
    distances
}

fn add_dependencies<'a>(
    crate_graph: &mut CrateGraphBuilder,
    package_ids: impl Iterator<Item = &'a PackageId>,
    nodes: &HashMap<&PackageId, &Node>,
    crate_ids: &HashMap<PackageId, CrateBuilderId>,
) {
    let mut dependencies = HashSet::new();
    for package_id in package_ids {
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
}

fn path_string(path: FilePath) -> String {
    path.as_path().to_string_lossy().into_owned()
}

fn discover_sysroot_library() -> Result<PathBuf, FsLoadError> {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let output = Command::new(&rustc)
        .args(["--print", "sysroot"])
        .output()
        .map_err(|source| FsLoadError::RunRustc {
            rustc: PathBuf::from(&rustc),
            source,
        })?;
    if !output.status.success() {
        return Err(FsLoadError::RustcFailed {
            rustc: PathBuf::from(&rustc),
            message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let sysroot = PathBuf::from(String::from_utf8(output.stdout)?.trim());

    // This follows rust-analyzer's sysroot discovery order.
    let library_dir = env::var_os("RUST_SRC_PATH")
        .map(PathBuf::from)
        .filter(|path| path.join("core").is_dir())
        .unwrap_or_else(|| sysroot.join("lib/rustlib/src/rust/library"));
    if library_dir.join("std/src/lib.rs").is_file() {
        Ok(library_dir)
    } else {
        Err(FsLoadError::MissingRustSrc { sysroot })
    }
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

fn real_file_path(path: &Path) -> Result<FilePath, FsLoadError> {
    let path = path
        .canonicalize()
        .map_err(|source| FsLoadError::CanonicalizePath {
            path: path.to_path_buf(),
            source,
        })?;
    let path = path
        .to_str()
        .ok_or_else(|| FsLoadError::NonUtf8Path { path: path.clone() })?;
    FilePath::new_real(path).map_err(|source| FsLoadError::FilePath { source })
}

fn edition(value: &str) -> Result<Edition, FsLoadError> {
    match value {
        "2015" => Ok(Edition::E2015),
        "2018" => Ok(Edition::E2018),
        "2021" => Ok(Edition::E2021),
        "2024" => Ok(Edition::E2024),
        value => Err(FsLoadError::UnsupportedEdition {
            edition: value.to_owned(),
        }),
    }
}

/// An error encountered while loading server filesystem state.
#[derive(Debug, Error, Diagnostic)]
pub enum FsLoadError {
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
    #[error("Cargo dependency graph references missing package {package_id}")]
    MissingPackage { package_id: String },
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
    #[error(transparent)]
    FilePath {
        #[from]
        source: crate::compiler::FilePathError,
    },
    #[error("unsupported Rust edition {edition}")]
    UnsupportedEdition { edition: String },
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
    #[error("command output is not valid UTF-8")]
    InvalidCommandOutput(#[from] std::string::FromUtf8Error),
}
