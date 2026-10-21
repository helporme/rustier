use std::path::{Path, PathBuf};

use crate::{
    compiler::{
        codegen::manifest::Manifest,
        db::Db,
        ir::{facade::FacadeModuleEntry, transport::backend::TransportBackend},
    },
    server::write_file::{WriteFileError, write_file},
};

use super::Project;

pub use crate::compiler::codegen::transport::js::WasmLoading;

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

    pub fn name(self, db: &'a dyn Db) -> &'static str {
        match self {
            Self::Node(crate_) => crate_.name(),
            Self::Wasm(crate_) => crate_.name(db),
        }
    }

    pub fn transport(self, db: &'a dyn Db) -> TransportBackend {
        match self {
            Self::Node(..) => TransportBackend::Napi,
            Self::Wasm(crate_) => TransportBackend::Wasm(crate_.transport(db)),
        }
    }

    /// Root directory of the bridge crate.
    #[salsa::tracked(returns(ref))]
    pub fn root_dir(self, db: &'a dyn Db) -> PathBuf {
        self.project(db)
            .out_dir(db)
            .join("crates")
            .join(self.name(db))
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
            "/../rustier-ext"
        )));

        let manifest = Manifest {
            name: self.name(db),
            source_package_name: &self.project(db).cfg(db).package_name,
            source_crate_name: self.project(db).src_crate(db).name(db),
            source_package_path: self.project(db).root_dir(db),
            runtime_path,
            transport: self.transport(db),
        };

        let lib = self
            .project(db)
            .reachable_facade(db)
            .runtime(db)
            .render_rust(db, self.transport(db))
            .clone();

        let build = manifest
            .transport
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
    fn js_module(self, db: &'db dyn Db, path: &str) -> Option<FacadeModuleEntry<'db>> {
        self.reachable_facade(db).modules(db).find(|module| {
            module.virtual_path(db) == path
                || module
                    .file_path(db)
                    .is_some_and(|file| file.as_str() == path)
        })
    }
}
