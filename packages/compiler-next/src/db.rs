use ra_ap_base_db::{
    self as base, CratesMap, FileId, FileSourceRootInput, FileText, Files, Nonce, SourceDatabase,
    SourceRoot, SourceRootId, SourceRootInput,
};
use salsa::Durability;
use triomphe::Arc;

mod virt_file;

pub(crate) use virt_file::VirtFile;
use virt_file::is_virt_file_id;

#[salsa::db]
pub trait Db: ra_ap_hir::db::HirDatabase {}

#[salsa::db]
#[derive(Clone)]
pub struct RootDatabase {
    storage: salsa::Storage<Self>,
    files: Arc<Files>,
    crates: Arc<CratesMap>,
    nonce: Nonce,
}

impl std::fmt::Debug for RootDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RootDatabase")
    }
}

#[salsa::db]
impl salsa::Database for RootDatabase {}

impl Default for RootDatabase {
    fn default() -> Self {
        let mut db = Self {
            storage: Default::default(),
            files: Default::default(),
            crates: Default::default(),
            nonce: Nonce::new(),
        };

        base::set_all_crates_with_durability(&mut db, [], Durability::HIGH);
        base::LibraryRoots::new(&db, Default::default());
        base::LocalRoots::new(&db, Default::default());
        ra_ap_hir::ProcMacros::init_default(&db, Durability::MEDIUM);
        ra_ap_hir::db::set_expand_proc_attr_macros(&mut db, false);

        db
    }
}

#[salsa::db]
impl Db for RootDatabase {}

#[salsa::db]
impl SourceDatabase for RootDatabase {
    fn file_text(&self, file: FileId) -> FileText {
        match VirtFile::try_from(file) {
            Ok(file) => file.text_input(self),
            Err(()) => self.files.file_text(file),
        }
    }

    fn source_root(&self, root: SourceRootId) -> SourceRootInput {
        match VirtFile::try_from(FileId::from_raw(root.0)) {
            Ok(file) => file.source_root(self),
            Err(()) => self.files.source_root(root),
        }
    }

    fn file_source_root(&self, file: FileId) -> FileSourceRootInput {
        match VirtFile::try_from(file) {
            Ok(file) => file.membership(self),
            Err(()) => self.files.file_source_root(self, file),
        }
    }

    fn set_file_text(&mut self, file: FileId, text: &str) {
        if is_virt_file_id(file) {
            panic!("cannot overwrite virtual file with dependency source");
        }

        let files = self.files.clone();
        files.set_file_text(self, file, text);
    }

    fn set_file_text_with_durability(&mut self, file: FileId, text: &str, durability: Durability) {
        if is_virt_file_id(file) {
            panic!("cannot overwrite virtual file with dependency source");
        }

        let files = self.files.clone();
        files.set_file_text_with_durability(self, file, text, durability);
    }

    fn set_file_source_root_with_durability(
        &mut self,
        file: FileId,
        root: SourceRootId,
        durability: Durability,
    ) {
        if is_virt_file_id(file) {
            panic!("cannot overwrite virtual file membership");
        }

        let files = self.files.clone();
        files.set_file_source_root_with_durability(self, file, root, durability);
    }

    fn set_source_root_with_durability(
        &mut self,
        root: SourceRootId,
        source: Arc<SourceRoot>,
        durability: Durability,
    ) {
        if is_virt_file_id(FileId::from_raw(root.0)) {
            panic!("cannot overwrite virtual source root");
        }

        let files = self.files.clone();
        files.set_source_root_with_durability(self, root, source, durability);
    }

    fn crates_map(&self) -> Arc<CratesMap> {
        self.crates.clone()
    }

    fn nonce_and_revision(&self) -> (Nonce, salsa::Revision) {
        (
            self.nonce,
            salsa::plumbing::ZalsaDatabase::zalsa(self).current_revision(),
        )
    }

    fn line_column(&self, file: FileId, offset: ra_ap_syntax::TextSize) -> Result<(u32, u32), ()> {
        ra_ap_ide_db::line_index(self, file)
            .try_line_col(offset)
            .map(|position| (position.line, position.col))
            .ok_or(())
    }
}
