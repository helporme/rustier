use ra_ap_base_db::{
    FileSourceRootInput, FileText, SourceDatabase, SourceRoot, SourceRootId, SourceRootInput,
};
use ra_ap_vfs::{FileId, VfsPath, file_set::FileSet};
use salsa::plumbing::{AsId, FromId};
use triomphe::Arc;

const VIRT_FILE_TAG: u32 = (ra_ap_ide_db::span::EditionedFileId::MAX_FILE_ID + 1) / 2;

#[salsa::interned(no_lifetime, revisions = usize::MAX)]
pub(crate) struct VirtFile {
    #[returns(ref)]
    text: Arc<str>,
}

#[salsa::tracked]
impl VirtFile {
    pub(crate) fn file_id(self) -> FileId {
        let bits = self.as_id().as_bits();

        if bits >= u64::from(VIRT_FILE_TAG) {
            panic!("virtual file ID space exhausted");
        }

        FileId::from_raw(VIRT_FILE_TAG | bits as u32)
    }

    #[salsa::tracked]
    pub(crate) fn text_input(self, db: &dyn SourceDatabase) -> FileText {
        FileText::new(db, self.text(db).clone(), self.file_id())
    }

    #[salsa::tracked]
    pub(crate) fn source_root(self, db: &dyn SourceDatabase) -> SourceRootInput {
        let file = self.file_id();
        let mut files = FileSet::default();

        files.insert(
            file,
            VfsPath::new_virtual_path(format!("/rustier/virtual/{}/lib.rs", file.index())),
        );

        SourceRootInput::new(db, Arc::new(SourceRoot::new_local(files)))
    }

    #[salsa::tracked]
    pub(crate) fn membership(self, db: &dyn SourceDatabase) -> FileSourceRootInput {
        FileSourceRootInput::new(db, SourceRootId(self.file_id().index()))
    }
}

impl TryFrom<FileId> for VirtFile {
    type Error = ();

    fn try_from(file: FileId) -> Result<Self, Self::Error> {
        if !is_virt_file_id(file) {
            return Err(());
        }

        let bits = u64::from(file.index() & !VIRT_FILE_TAG);
        let id = salsa::Id::from_bits(bits);

        Ok(Self::from_id(id))
    }
}

pub(super) fn is_virt_file_id(file: FileId) -> bool {
    file.index() & VIRT_FILE_TAG != 0
}
