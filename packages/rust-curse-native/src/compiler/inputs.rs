use std::path::PathBuf;

use salsa::Setter;

use crate::compiler::db::Db;

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct CompilationMetadata {
    pub bin_path: PathBuf,
    pub js_ffi_path: PathBuf,
    pub js_mods_root_path: PathBuf,
}

impl CompilationMetadata {
    pub fn set(db: &mut dyn Db, value: Self) {
        CompilationMetadataInput::set(db, value);
    }

    pub fn get(db: &dyn Db) -> Self {
        CompilationMetadataInput::get(db).inner(db)
    }
}

#[salsa::input(singleton)]
struct CompilationMetadataInput {
    pub inner: CompilationMetadata,
}

impl CompilationMetadataInput {
    fn set(db: &mut dyn Db, inner: CompilationMetadata) -> Self {
        if let Some(input) = Self::try_get(db) {
            input.set_inner(db).to(inner);
            input
        } else {
            Self::new(db, inner)
        }
    }
}
