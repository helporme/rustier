//! Shared compiler test fixtures.

use crate::compiler::{
    Compiler, CompilerOpts, CrateApiDiscovery, CrateApiImportMode, CrateGraphBuilder, Edition,
    FfiLayout, FilePath, Platform, WorkspaceCrate, db::RootDatabase,
};

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
    let mut compiler = Compiler::new(CompilerOpts {
        platform: Platform::Node,
        ffi_layout: FfiLayout {
            ffi_js_file: FilePath::new_virtual("/generated/ffi.js")
                .expect("test FFI file should be a valid path"),
        },
    });

    compiler
        .change_workspace(|change| {
            change.set_file(root_file.clone(), Some(source));
            for (path, source) in files {
                let path = FilePath::new_virtual(format!("/src/{path}"))
                    .expect("test module should have a valid virtual path");
                change.set_file(path, Some(source.clone()));
            }

            let mut crates = CrateGraphBuilder::default();
            crates.add_crate(WorkspaceCrate {
                name: "kitty_go".to_owned(),
                edition: Edition::E2024,
                root_file,
                version: "0.0.0".to_owned(),
                api_discovery: CrateApiDiscovery::Eager,
                api_import_mode: CrateApiImportMode::Mod,
            });
            change.set_crates(crates);
            Ok(())
        })
        .expect("build test compiler workspace");

    compiler.db
}

/// Creates a compiler database containing rst-analyzer inputs for inline Rust
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
