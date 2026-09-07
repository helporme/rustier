//! Additional Salsa inputs used by the compiler.

use std::collections::HashSet;

use salsa::Setter as _;

use crate::compiler::{db::Db, path::FilePath};

/// Root file paths of crates treated as local by the compiler.
///
/// All crates are external until this input is set.
/// Crates sharing a root file share the same locality.
#[salsa::input(singleton)]
pub struct LocalCrates {
    #[returns(ref)]
    root_files: HashSet<FilePath>,
}

impl LocalCrates {
    /// Replaces the set of local crate root files.
    pub fn set(db: &mut dyn Db, root_files: impl IntoIterator<Item = FilePath>) {
        let input = Self::try_get(db).unwrap_or_else(|| Self::new(db, HashSet::new()));

        input
            .set_root_files(db)
            .to(root_files.into_iter().collect());
    }

    pub(super) fn contains(db: &dyn Db, root_file: &FilePath) -> bool {
        match Self::try_get(db) {
            Some(input) => input.root_files(db).contains(root_file),
            None => {
                // Salsa does not track the absence of a singleton.
                db.report_untracked_read();
                false
            }
        }
    }
}
