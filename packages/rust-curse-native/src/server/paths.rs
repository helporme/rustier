use std::path::PathBuf;

use salsa::Durability;

use crate::server::db::Db;

/// A workspace paths on which [`server`](super) operates.
#[salsa::input(singleton, debug)]
pub struct WorkspacePaths {
    /// Absolute path to the manifest file of the target crate.
    pub manifest_path: PathBuf,
    /// Absolute path to the output directory (`.rust_curse`).
    pub out_dir: PathBuf,
}

#[salsa::tracked]
impl WorkspacePaths {
    pub fn set(db: &mut dyn Db, manifest_path: PathBuf, out_dir: PathBuf) -> Self {
        use salsa::Setter;
        if let Some(input) = Self::try_get(db) {
            input
                .set_manifest_path(db)
                .with_durability(Durability::HIGH)
                .to(manifest_path);
            input
                .set_out_dir(db)
                .with_durability(Durability::HIGH)
                .to(out_dir);
            input
        } else {
            Self::new(db, manifest_path, out_dir)
        }
    }

    /// Path to the local tools output directory.
    #[salsa::tracked(returns(ref))]
    pub fn out_tools_dir(self, db: &dyn Db) -> PathBuf {
        self.out_dir(db).join("tools")
    }

    /// Path to the crates output directory.
    #[salsa::tracked(returns(ref))]
    pub fn out_crates_dir(self, db: &dyn Db) -> PathBuf {
        self.out_dir(db).join("crates")
    }

    /// Path to the types output directory.
    #[salsa::tracked(returns(ref))]
    pub fn out_types_dir(self, db: &dyn Db) -> PathBuf {
        self.out_dir(db).join("types")
    }
}
