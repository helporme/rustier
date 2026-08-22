//! Additional inputs used by compiler besides those provided by rust-analyzer.

use std::path::PathBuf;

use crate::compiler::db::Db;

/// Target platform used for FFI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum Platform {
    Wasm { target: WasmTarget },
    Node,
}

/// Wasm execution environment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum WasmTarget {
    Web,
    Node,
}

/// Layout of generated ffi files
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct FfiLayout {
    /// Location of the js file exposing all ffi functions
    pub ffi_js_file: PathBuf,
}

macro_rules! impl_salsa_input {
    ($name:ident, $input_name:ident) => {
        impl $name {
            pub(super) fn set(db: &mut dyn Db, value: Self) {
                $input_name::set(db, value);
            }

            pub(super) fn get(db: &dyn Db) -> Self {
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
