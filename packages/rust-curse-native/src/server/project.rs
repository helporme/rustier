use std::path::{Path, PathBuf};

use crate::compiler::{
    codegen::shim::Platform,
    db::Db,
    ir::{
        rust::{Crate, Rust, usages::UsageGraph},
        shim::Shim,
    },
};

pub mod bridge_crate;
pub mod ts;

pub use bridge_crate::{BridgeCrate, BridgeCrateNode, BridgeCrateWasm, WasmLoading};
use ts::ProjectTs;

#[salsa::interned(debug)]
pub struct Project<'db> {
    #[returns(ref)]
    cfg: ProjectConfig,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct ProjectConfig {
    /// Manifest path of the project crate
    pub manifest_path: PathBuf,
    /// Path to the root file of the project (lib.rs)
    pub root_file: PathBuf,
    /// Cargo package name, which can differ from the Rust library target name.
    pub package_name: String,
}

impl<'a> Project<'a> {
    /// Bridge crate of the project.
    pub fn bridge_crate(self, db: &'a dyn Db, platform: Platform) -> BridgeCrate<'a> {
        match platform {
            Platform::Node => BridgeCrate::Node(BridgeCrateNode::new(db, self)),
            Platform::Wasm => BridgeCrate::Wasm(BridgeCrateWasm::new(db, self)),
        }
    }

    /// Source crate of the project.
    pub fn src_crate(self, db: &'a dyn Db) -> Crate<'a> {
        *Rust::get(db)
            .crates(db)
            .iter()
            .find(|c| {
                c.is_local(db) && c.root(db).file_path(db).as_path() == self.cfg(db).root_file
            })
            .expect("Project must have a local crate")
    }

    /// A shim reachable from the project's local crate.
    pub fn reachable_shim(self, db: &'a dyn Db) -> Shim<'a> {
        Rust::get(db).shim_reachable_from(db, UsageGraph::new(db, [self.src_crate(db).root(db)]))
    }

    /// Project root directory.
    pub fn root_dir(self, db: &'a dyn Db) -> &'a Path {
        self.cfg(db)
            .manifest_path
            .parent()
            .expect("Cargo.toml must be in a directory")
    }

    /// Output directory.
    pub fn out_dir(self, db: &'a dyn Db) -> PathBuf {
        self.root_dir(db).join(".rust-curse")
    }

    /// Typescript configuration of the project.
    pub fn ts(self) -> ProjectTs<'a> {
        ProjectTs::new(self)
    }
}
