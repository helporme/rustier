//! Additional salsa inputs used by compiler besides those provided by
//! rust-analyzer.

use std::collections::HashMap;

use crate::compiler::{db::Db, path::FilePath};

/// Target platform used for FFI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
#[non_exhaustive]
pub enum Platform {
    Wasm { target: WasmEnv },
    Node,
}

/// Wasm execution environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
#[non_exhaustive]
pub enum WasmEnv {
    Web,
    Node,
}

/// Layout of generated ffi files
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct FfiLayout {
    /// Location of the js file exposing all ffi functions
    pub ffi_js_file: FilePath,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct CratesMetadata {
    /// A crate metadata mapped by crate root file path (virutal or not).
    pub crates: HashMap<FilePath, CrateMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct CrateMetadata {
    pub api_discovery: CrateApiDiscovery,
    pub api_import_mode: CrateApiImportMode,
}

/// How to discover the crate public API to compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::Update)]
pub enum CrateApiDiscovery {
    /// Compile all public API of this crate.
    Eager,
    /// Compile only public API referenced by eager crates.
    Lazy,
}

/// Shape of imports generated for the crate modules
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum CrateApiImportMode {
    /// Genearte js imports matching the rust module path.
    Mod,
    /// Generate js imports to files ignoring the rust module path.
    Relative,
}

macro_rules! impl_salsa_input {
    ($name:ident, $input_name:ident) => {
        impl $name {
            /// Sets this compiler input in the database.
            pub fn set(db: &mut dyn Db, value: Self) {
                $input_name::set(db, value);
            }

            /// Gets this compiler input from the database.
            pub fn get(db: &dyn Db) -> Self {
                $input_name::get(db).inner(db)
            }
        }

        #[salsa::input(singleton)]
        struct $input_name {
            pub inner: $name,
        }

        impl $input_name {
            fn set(db: &mut dyn Db, inner: $name) -> Self {
                use salsa::Setter;
                if let Some(input) = Self::try_get(db) {
                    input.set_inner(db).to(inner);
                    input
                } else {
                    Self::new(db, inner)
                }
            }
        }
    };
}

impl_salsa_input!(Platform, PlatformInput);
impl_salsa_input!(FfiLayout, FfiLayoutInput);
impl_salsa_input!(CratesMetadata, CratesMetadataInput);
