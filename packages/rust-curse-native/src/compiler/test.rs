//! Shared compiler test fixtures.

use std::collections::HashMap;

use ra_ap_test_fixture::ChangeFixture;

use crate::compiler::{
    db::RootDatabase,
    inputs::{
        CrateApiDiscovery, CrateApiImportMode, CrateMetadata, CratesMetadata, FfiLayout, Platform,
    },
    path::FilePath,
};

const TEST_CRATE_FIXTURE_HEADER: &str = r#"//- minicore: clone, derive
//- /src/lib.rs crate:test_crate deps:std edition:2024
"#;

const TEST_STD_FIXTURE: &str = r#"
//- /sysroot/std/lib.rs crate:std new_source_root:library
pub mod string {
    pub struct String;

    impl core::clone::Clone for String {
        fn clone(&self) -> Self {
            loop {}
        }
    }
}

pub mod prelude {
    pub mod rust_2024 {
        pub use core::prelude::rust_2024::*;
        pub use crate::string::String;
    }
}
"#;

#[doc(hidden)]
pub(super) fn format_rust_source(source: &str) -> String {
    syn::parse_file(source)
        .map(|file| prettyplease::unparse(&file))
        .unwrap_or_else(|_| source.to_owned())
}

#[doc(hidden)]
pub(super) fn build_test_db(source: String, files: &[(&str, String)]) -> RootDatabase {
    let root_file =
        FilePath::new_virtual("/src/lib.rs").expect("test crate root should be a valid path");

    let mut fixture = String::from(TEST_CRATE_FIXTURE_HEADER);
    fixture.push_str(&source);
    fixture.push('\n');
    for (path, source) in files {
        fixture.push_str("\n//- /src/");
        fixture.push_str(path);
        fixture.push('\n');
        fixture.push_str(source);
        fixture.push('\n');
    }
    fixture.push_str(TEST_STD_FIXTURE);

    let mut db = RootDatabase::default();
    db.apply_change(ChangeFixture::parse(&fixture).change);
    Platform::set(&mut db, Platform::Node);
    FfiLayout::set(
        &mut db,
        FfiLayout {
            ffi_js_file: FilePath::new_virtual("/generated/ffi.js")
                .expect("test FFI file should be a valid path"),
        },
    );
    CratesMetadata::set(
        &mut db,
        CratesMetadata {
            crates: HashMap::from([(
                root_file,
                CrateMetadata {
                    api_discovery: CrateApiDiscovery::Eager,
                    api_import_mode: CrateApiImportMode::Mod,
                },
            )]),
        },
    );

    db
}

/// Creates a compiler database containing rust-analyzer inputs for inline Rust
/// code.
///
/// # Example
///
/// ```ignore
/// let db = test_db! {
///     pub struct User {
///         pub name: String,
///     }
///
///     pub fn greet(user: &User) -> String {
///         format!("Hello, {}!", user.name)
///     }
/// };
/// ```
///
/// A module can be placed in a separate virtual source file with `@file`:
///
/// ```ignore
/// let db = test_db! {
///     @file: "models.rs";
///     pub mod models {
///         pub struct User {
///             pub name: String,
///         }
///     }
///
///     pub fn greet(user: &models::User) -> String {
///         format!("Hello, {}!", user.name)
///     }
/// };
/// ```
#[macro_export]
macro_rules! test_db {
    (
        @parse;
        @source: [$($source:tt)*];
        @bodies: [$($bodies:tt)*];
        @file: $path:literal;
        $visibility:vis mod $name:ident { $($body:tt)* }
        $($rest:tt)*
    ) => {
        $crate::test_db! {
            @parse;
            @source: [
                $($source)*
                #[path = $path]
                $visibility mod $name;
            ];
            @bodies: [
                $($bodies)*
                ($path, { $($body)* })
            ];
            $($rest)*
        }
    };
    (
        @parse;
        @source: [$($source:tt)*];
        @bodies: [$($bodies:tt)*];
        $token:tt
        $($rest:tt)*
    ) => {
        $crate::test_db! {
            @parse;
            @source: [$($source)* $token];
            @bodies: [$($bodies)*];
            $($rest)*
        }
    };
    (
        @parse;
        @source: [$($source:tt)*];
        @bodies: [$(($path:literal, { $($body:tt)* }))*];
    ) => {{
        let __source = $crate::compiler::test::format_rust_source(
            ::std::stringify!($($source)*),
        );
        let __files: &[(&str, ::std::string::String)] = &[$(
            (
                $path,
                $crate::compiler::test::format_rust_source(
                    ::std::stringify!($($body)*),
                ),
            )
        ),*];
        $crate::compiler::test::build_test_db(__source, __files)
    }};
    ($($source:tt)*) => {
        $crate::test_db! {
            @parse;
            @source: [];
            @bodies: [];
            $($source)*
        }
    };
}
