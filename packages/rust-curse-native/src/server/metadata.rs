use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use cargo_metadata::{MetadataCommand, PackageId, TargetKind};
use thiserror::Error;

use crate::{
    compiler::workspace::{CrateGraphBuilder, Edition, WorkspaceCrate, WorkspaceCrateOrigin},
    server::db::Db,
};

/// Cargo package graph used to populate a compiler workspace.
#[salsa::input(singleton, debug)]
pub struct Metadata {
    inner: Arc<cargo_metadata::Metadata>,
    #[returns(ref)]
    manifest_path: PathBuf,
}

#[salsa::tracked]
impl Metadata {
    /// Loads Cargo metadata for `manifest_path` and stores its crate graph.
    pub fn load(db: &mut dyn Db, manifest_path: PathBuf) -> Result<Self, MetadataLoadError> {
        use salsa::Setter as _;

        let cargo_metadata = MetadataCommand::new()
            .manifest_path(&manifest_path)
            .exec()
            .map_err(|source| MetadataLoadError::CargoMetadata {
                manifest_path: manifest_path.clone(),
                source,
            })?;

        if let Some(metadata) = Metadata::try_get(db) {
            metadata.set_inner(db).to(Arc::new(cargo_metadata));
            metadata.set_manifest_path(db).to(manifest_path);
            Ok(metadata)
        } else {
            Ok(Self::new(db, Arc::new(cargo_metadata), manifest_path))
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn crates(self, db: &dyn Db) -> Vec<MetadataCrate<'_>> {
        let inner = self.inner(db);
        inner
            .packages
            .iter()
            .map(|package| MetadataCrate::new(db, Arc::clone(&inner), &package.id))
            .collect()
    }

    #[salsa::tracked]
    pub fn main_crate(self, db: &dyn Db) -> Option<MetadataCrate<'_>> {
        self.crates(db)
            .iter()
            .copied()
            .find(|crate_| crate_.manifest_path(db) == self.manifest_path(db))
    }
}

impl Metadata {
    /// Creates a [`CrateGraphBuilder`] for the Cargo dependency graph.
    pub fn crate_garph_builder(self, db: &dyn Db) -> CrateGraphBuilder {
        let workspace_crates = self
            .crates(db)
            .iter()
            // todo: check if it's fine to load only lib libraries
            .filter_map(|crate_| {
                Some(WorkspaceCrate {
                    name: crate_.name(db).to_owned(),
                    edition: crate_.edition(db),
                    root_file: crate_.lib_file(db)?.try_into().ok()?,
                    version: crate_.version(db).to_owned(),
                    origin: if crate_.is_in_workspace(db) {
                        WorkspaceCrateOrigin::Local
                    } else {
                        WorkspaceCrateOrigin::Library
                    },
                    api_import_mode: todo!(),
                })
            })
            .collect::<Vec<_>>();

        let mut graph = CrateGraphBuilder::default();

        graph
    }
}

/// A library crate in the Cargo dependency graph.
#[salsa::interned(debug)]
pub struct MetadataCrate<'db> {
    #[returns(ref)]
    inner: Arc<cargo_metadata::Metadata>,
    #[returns(ref)]
    id: PackageId,
}

#[salsa::tracked]
impl<'db> MetadataCrate<'db> {
    /// Rust crate name, which may differ from the Cargo package name.
    pub fn name(self, db: &'db dyn Db) -> &'db str {
        let package = self.package(db);
        package.name.as_str()
    }

    /// The edition of the crate.
    pub fn edition(self, db: &dyn Db) -> Edition {
        let package = self.package(db);

        match package.edition {
            cargo_metadata::Edition::E2015 => Edition::E2015,
            cargo_metadata::Edition::E2018 => Edition::E2018,
            cargo_metadata::Edition::E2021 => Edition::E2021,
            cargo_metadata::Edition::E2024 => Edition::E2024,
            _ => Edition::E2024,
        }
    }

    /// The path to the library main file (`lib.rs`) or None if the
    /// crate is not a library.
    pub fn lib_file(self, db: &'db dyn Db) -> Option<&'db Path> {
        let package = self.package(db);
        package
            .targets
            .iter()
            .find(|t| {
                t.kind
                    .iter()
                    .any(|k| matches!(k, TargetKind::Lib | TargetKind::RLib | TargetKind::DyLib))
            })
            .map(|t| t.src_path.as_std_path())
    }

    /// The version of the crate.
    #[salsa::tracked(returns(ref))]
    pub fn version(self, db: &dyn Db) -> String {
        self.package(db).version.to_string()
    }

    /// The path to the manifest file.
    pub fn manifest_path(self, db: &'db dyn Db) -> &'db Path {
        self.package(db).manifest_path.as_std_path()
    }

    /// Whether the crate is a member of the workspace.
    pub fn is_in_workspace(self, db: &dyn Db) -> bool {
        let metadata = self.inner(db);
        metadata.workspace_members.contains(self.id(db))
    }

    #[salsa::tracked(returns(ref))]
    fn package(self, db: &dyn Db) -> cargo_metadata::Package {
        let id = self.id(db);

        self.inner(db)
            .packages
            .iter()
            .find(|package| &package.id == id)
            .unwrap()
            .clone()
    }
}

/// An error encountered while loading a Cargo crate graph.
#[derive(Debug, Error, miette::Diagnostic)]
pub enum MetadataLoadError {
    #[error("failed to read Cargo metadata for {}", manifest_path.display())]
    CargoMetadata {
        manifest_path: PathBuf,
        #[source]
        source: cargo_metadata::Error,
    },
}

// /// Converts this metadata into the crate graph accepted by the compiler
// /// workspace.
// pub fn crate_graph(self, db: &dyn Db) -> CrateGraphBuilder {
//     let mut graph = CrateGraphBuilder::default();
//     let ids = self
//         .crates(db)
//         .iter()
//         .map(|crate_| {
//             let id = graph.add_crate(WorkspaceCrate {
//                 name: crate_.name(db).to_owned(),
//                 edition: crate_.edition(db),
//                 root_file: FilePath::new_real(
//                     crate_.root_file(db).to_string_lossy().into_owned(),
//                 )
//                 .expect("Cargo metadata returned a relative crate root"),
//                 version: crate_.version(db).to_owned(),
//                 kind: crate_.kind(db, self),
//                 api_import_mode: crate_.api_import_mode(db, self),
//             });
//             (*crate_, id)
//         })
//         .collect::<HashMap<_, _>>();

//     for dependency in self.dependencies(db) {
//         let Some(&from_crate) = ids.get(&dependency.source(db)) else {
//             continue;
//         };
//         let Some(&to_crate) = ids.get(&dependency.target(db)) else {
//             continue;
//         };
//         graph.add_dependency(WorkspaceCrateDependency {
//             name: dependency.alias(db).to_owned(),
//             from_crate,
//             to_crate,
//         });
//     }

//     graph
// }

// fn dependency_distances(
//     root: &PackageId,
//     nodes: &HashMap<&PackageId, &Node>,
// ) -> HashMap<PackageId, usize> {
//     let mut distances = HashMap::from([(root.clone(), 0)]);
//     let mut pending = VecDeque::from([root.clone()]);
//     while let Some(package_id) = pending.pop_front() {
//         let distance = distances[&package_id];
//         let Some(node) = nodes.get(&package_id) else {
//             continue;
//         };
//         for dependency in node.deps.iter().filter(|dependency|
// is_normal(dependency)) {             if
// distances.contains_key(&dependency.pkg) {                 continue;
//             }
//             distances.insert(dependency.pkg.clone(), distance + 1);
//             pending.push_back(dependency.pkg.clone());
//         }
//     }
//     distances
// }

// fn library_target(package: &Package) -> Option<&Target> {
//     package.targets.iter().find(|target| {
//         target.kind.iter().any(|kind| {
//             matches!(
//                 kind,
//                 TargetKind::CDyLib
//                     | TargetKind::DyLib
//                     | TargetKind::Lib
//                     | TargetKind::ProcMacro
//                     | TargetKind::RLib
//                     | TargetKind::StaticLib
//             )
//         })
//     })
// }

// fn is_normal(dependency: &NodeDep) -> bool {
//     dependency
//         .dep_kinds
//         .iter()
//         .any(|kind| kind.kind == DependencyKind::Normal)
// }

// fn edition_from_str(value: &str) -> Result<Edition, MetadataLoadError> {
//     match value {
//         "2015" => Ok(Edition::E2015),
//         "2018" => Ok(Edition::E2018),
//         "2021" => Ok(Edition::E2021),
//         "2024" => Ok(Edition::E2024),
//         value => Err(MetadataLoadError::UnsupportedEdition {
//             edition: value.to_owned(),
//         }),
//     }
// }

// fn canonicalize(path: &Path) -> Result<PathBuf, MetadataLoadError> {
//     path.canonicalize()
//         .map_err(|source| MetadataLoadError::CanonicalizeManifest {
//             manifest_path: path.to_path_buf(),
//             source,
//         })
// }
