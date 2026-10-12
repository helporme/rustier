use std::path::{Path, PathBuf};

use crate::{
    compiler::{
        codegen::runtime::{Platform, manifest::Manifest},
        db::Db,
        ir::facade::FacadeModule,
    },
    server::write_file::{WriteFileError, write_file},
};

use super::Project;

pub use crate::compiler::codegen::ffi::WasmLoading;

mod common;
mod node;
mod wasm;

pub use node::{BridgeCrateNode, NodeBuild};
pub use wasm::{BridgeCrateWasm, WasmBuild};

/// A glue crate that contains rendered bindings code from source rust crate
/// to the JavaScript.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum BridgeCrate<'a> {
    Node(BridgeCrateNode<'a>),
    Wasm(BridgeCrateWasm<'a>),
}

#[salsa::tracked]
impl<'a> BridgeCrate<'a> {
    pub fn project(self, db: &'a dyn Db) -> Project<'a> {
        match self {
            Self::Node(crate_) => crate_.project(db),
            Self::Wasm(crate_) => crate_.project(db),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Node(crate_) => crate_.name(),
            Self::Wasm(crate_) => crate_.name(),
        }
    }

    pub const fn platform(self) -> Platform {
        match self {
            Self::Node(..) => Platform::Node,
            Self::Wasm(..) => Platform::Wasm,
        }
    }

    /// Root directory of the bridge crate.
    #[salsa::tracked(returns(ref))]
    pub fn root_dir(self, db: &'a dyn Db) -> PathBuf {
        self.project(db)
            .out_dir(db)
            .join("crates")
            .join(self.name())
    }

    /// Path to the manifest file of the bridge crate.
    #[salsa::tracked(returns(ref))]
    pub fn manifest_path(self, db: &'a dyn Db) -> PathBuf {
        self.root_dir(db).join("Cargo.toml")
    }

    /// Synchronization of the bridge crate to the output directory.
    ///
    /// Doesn't do anything until executed.
    pub fn sync(self, db: &'a dyn Db) -> BridgeCrateSync {
        let runtime_path = cfg!(feature = "local-runtime").then_some(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../rustier-rt"
        )));

        let manifest = Manifest {
            name: self.name(),
            source_package_name: &self.project(db).cfg(db).package_name,
            source_crate_name: self.project(db).src_crate(db).name(db),
            source_package_path: self.project(db).root_dir(db),
            runtime_path,
            platform: self.platform(),
        };

        let lib = self
            .project(db)
            .reachable_interface(db)
            .runtime(db)
            .render(db, self.platform())
            .clone();

        let build = manifest
            .platform
            .build_script_body()
            .map(|body| format!("fn main() {{ {body} }}\n"));

        BridgeCrateSync {
            out_dir: self.root_dir(db).clone(),
            manifest: manifest.render(),
            lib,
            build,
        }
    }

    fn dependency_features(self, db: &'a dyn Db, dep_features: &[String]) -> Vec<String> {
        // Match the dependency key used by Manifest::render, not the package name.
        let dependency = self.project(db).src_crate(db).name(db);
        dep_features
            .iter()
            .map(|feature| format!("{dependency}/{feature}"))
            .collect()
    }
}

/// Synchronization of the bridge crate to the output directory.
#[derive(Debug, Clone)]
pub struct BridgeCrateSync {
    out_dir: PathBuf,
    manifest: String,
    lib: String,
    build: Option<String>,
}

impl BridgeCrateSync {
    pub fn exec(&self) -> Result<(), WriteFileError> {
        let BridgeCrateSync {
            out_dir,
            manifest,
            lib,
            build,
        } = self;

        write_file(out_dir.join("Cargo.toml"), manifest)?;
        write_file(out_dir.join("src/lib.rs"), lib)?;
        if let Some(build) = build {
            write_file(out_dir.join("build.rs"), build)?;
        }
        Ok(())
    }
}

impl<'db> Project<'db> {
    fn js_module(self, db: &'db dyn Db, path: &str) -> Option<FacadeModule<'db>> {
        self.reachable_interface(db)
            .crates(db)
            .flat_map(|c| c.all_modules(db))
            .find(|m| {
                m.decals(db).virtual_path(db) == path || m.src(db).file_path(db).as_str() == path
            })
            .map(|m| m.facade(db))
    }
}
