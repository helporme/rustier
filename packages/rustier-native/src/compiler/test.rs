//! Shared compiler test fixtures.

use ra_ap_test_fixture::ChangeFixture;

use crate::compiler::db::RootDatabase;

const TEST_CRATE_FIXTURE_HEADER: &str = r#"//- /src/lib.rs crate:test_crate deps:std edition:2024
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
pub(super) fn build_test_db(
    source: String,
    files: &[(&str, String)],
    minicore: &str,
) -> RootDatabase {
    let mut fixture = format!("//- minicore: {minicore}\n{TEST_CRATE_FIXTURE_HEADER}");
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

    db
}

pub(super) fn build_js_env_test_db(source: &str) -> RootDatabase {
    let source = format_rust_source(source);
    let fixture = format!(
        "//- minicore: clone, derive\n\
         //- /src/lib.rs crate:test_crate deps:std,rustier_ext edition:2024\n\
         {source}\n\
         {TEST_STD_FIXTURE}\n\
         //- /rustier_ext/lib.rs crate:rustier_ext new_source_root:library edition:2024\n\
         pub mod runtime {{ pub mod js {{ pub struct JsEnv; }} }}\n",
    );
    let mut db = RootDatabase::default();

    db.apply_change(ChangeFixture::parse(&fixture).change);

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
        @minicore: $minicore:literal;
        @source: [$($source:tt)*];
        @bodies: [$($bodies:tt)*];
        @file: $path:literal;
        $visibility:vis mod $name:ident { $($body:tt)* }
        $($rest:tt)*
    ) => {
        $crate::test_db! {
            @parse;
            @minicore: $minicore;
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
        @minicore: $minicore:literal;
        @source: [$($source:tt)*];
        @bodies: [$($bodies:tt)*];
        $token:tt
        $($rest:tt)*
    ) => {
        $crate::test_db! {
            @parse;
            @minicore: $minicore;
            @source: [$($source)* $token];
            @bodies: [$($bodies)*];
            $($rest)*
        }
    };
    (
        @parse;
        @minicore: $minicore:literal;
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
        $crate::compiler::test::build_test_db(__source, __files, $minicore)
    }};
    (@minicore: $minicore:literal; $($source:tt)*) => {
        $crate::test_db! {
            @parse;
            @minicore: $minicore;
            @source: [];
            @bodies: [];
            $($source)*
        }
    };
    ($($source:tt)*) => {
        $crate::test_db! {
            @parse;
            @minicore: "clone, derive";
            @source: [];
            @bodies: [];
            $($source)*
        }
    };
}
