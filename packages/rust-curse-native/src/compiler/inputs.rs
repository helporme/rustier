//! Additional Salsa inputs used by the compiler besides those provided by
//! rust-analyzer.

use crate::compiler::{db::Db, path::FilePath};

/// Target platform used for FFI.
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

/// Layout of generated FFI files.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct FfiLayout {
    /// Location of the JavaScript file exposing all FFI functions.
    pub ffi_js_file: FilePath,
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
