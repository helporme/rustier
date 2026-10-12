use std::path::{Path, PathBuf};

use crate::compiler::{
    codegen::abi::AbiTransport,
    db::Db,
    ir::{
        interface::Interface,
        rust::{Crate, Rust, usages::UsageGraph},
    },
};

pub mod bridge_crate;
pub mod ts;

pub use bridge_crate::{
    BridgeCrate, BridgeCrateNode, BridgeCrateWasm, NodeBuild, WasmBuild, WasmLoading,
};
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
    pub fn bridge_crate(self, db: &'a dyn Db, transport: AbiTransport) -> BridgeCrate<'a> {
        match transport {
            AbiTransport::Napi => BridgeCrate::Node(BridgeCrateNode::new(db, self)),
            AbiTransport::WasmBindgen => BridgeCrate::Wasm(BridgeCrateWasm::new(db, self)),
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

    /// An interface reachable from the project's local crate.
    pub fn reachable_interface(self, db: &'a dyn Db) -> Interface<'a> {
        Rust::get(db)
            .interface_reachable_from(db, UsageGraph::new(db, [self.src_crate(db).root(db)]))
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
        self.root_dir(db).join(".rustier")
    }

    /// Typescript configuration of the project.
    pub fn ts(self) -> ProjectTs<'a> {
        ProjectTs::new(self)
    }
}
