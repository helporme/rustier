//! Test-only helpers for constructing fully initialized codegen databases.

// This intentionally mirrors `tests/codegen/utils/rustdoc.rs` since It's easier
// to copy 100 lines than establish some feature flags to allow this code be
// available from integration tests.

// todo: it's experimental module. The purpose of it is to have unit tests near
// the private code and better rustdoc examples. It might be horrible idea to
// create a temp crate per unit test, but how do I know if I won't try?

use crate::codegen::{
    db::DbImpl,
    ir::{ItemId, ItemPath},
    rustdoc::RustdocId,
};
pub use crate::test_db;

#[doc(hidden)]
pub const TEST_CRATE_NAME: &str = "rustdoc_test_crate";

/// Formats a tokenized Rust source file using standard Rust indentation.
#[doc(hidden)]
pub fn format_rust_source(source: &str) -> String {
    match syn::parse_file(source) {
        Ok(file) => prettyplease::unparse(&file),
        Err(_) => source.to_string(),
    }
}

/// Converts Rust tokens into a consistently formatted source string.
#[doc(hidden)]
#[macro_export]
macro_rules! rust_source {
    ($($source:tt)*) => {
        $crate::codegen::test::format_rust_source(::std::stringify!($($source)*))
    };
}

impl DbImpl {
    /// Returns the item ID for an item at a path relative to the test crate
    /// root.
    ///
    /// An empty path returns the test crate's root ID.
    ///
    /// ```no_run
    /// use rust_curse::{
    ///     codegen::api::{Api, ApiKind},
    ///     test_db,
    /// };
    ///
    /// let mut db = test_db! {};
    /// Api::set(&mut db, ApiKind::Node);
    /// let root = db.ir_id("");
    /// assert_eq!(root.path(&db), &["rustdoc_test_crate"]);
    /// ```
    pub fn item_id(&self, path: &str) -> ItemId<'_> {
        self.rustdoc_id(path).item_id(self)
    }

    /// Returns the rustdoc ID for an item at `path`, panicking if it does not
    /// exist.
    ///
    /// ```no_run
    /// use rust_curse::{
    ///     codegen::api::{Api, ApiKind},
    ///     test_db,
    /// };
    ///
    /// let mut db = test_db! {
    ///     pub mod path {
    ///         pub fn item() {}
    ///     }
    /// };
    /// Api::set(&mut db, ApiKind::Node);
    /// let id = db.rustdoc_id("path::item");
    /// assert_eq!(id.item(&db).name.as_deref(), Some("item"));
    /// ```
    pub fn rustdoc_id(&self, path: &str) -> RustdocId<'_> {
        let mut segments = vec![TEST_CRATE_NAME.to_string()];
        if !path.is_empty() {
            segments.extend(path.split("::").map(str::to_string));
        }

        RustdocId::find_by_path(self, &ItemPath::new(self, &segments))
            .expect("no rustdoc item found")
    }
}

/// Builds a codegen database from root source and additional module files.
#[doc(hidden)]
pub fn build_test_db(src: String, files: &[(&str, String)]) -> DbImpl {
    let fixture_dir = tempfile::tempdir().expect("create test crate tempdir");
    let crate_dir = fixture_dir.path().to_path_buf();
    let src_dir = crate_dir.join("src");

    std::fs::create_dir_all(&src_dir).expect("create test crate src directory");
    std::fs::write(
        crate_dir.join("Cargo.toml"),
        indoc::formatdoc! {r#"
            [package]
            name = "{TEST_CRATE_NAME}"
            version = "0.0.0"
            edition = "2021"
            publish = false

            [lib]
            path = "src/lib.rs"

            [workspace]
        "#},
    )
    .expect("write test crate manifest");
    std::fs::write(src_dir.join("lib.rs"), src).expect("write test crate source");

    for (path, source) in files {
        let file_path = src_dir.join(path);
        std::fs::create_dir_all(
            file_path
                .parent()
                .expect("test source file should have a parent directory"),
        )
        .expect("create test source file directory");
        std::fs::write(file_path, source).expect("write test source file");
    }

    let crate_body = crate::tools::rustdoc::run(&crate_dir.join("Cargo.toml"))
        .unwrap_or_else(|err| panic!("rustdoc generation failed: {err}"));
    let crates = std::collections::BTreeMap::from([(
        TEST_CRATE_NAME.to_string(),
        crate::codegen::rustdoc::Crate {
            body: crate_body,
            is_external: false,
            dir: crate_dir,
        },
    )]);

    let mut db = DbImpl::default();
    crate::codegen::rustdoc::RustdocCrates::set(&mut db, crates);

    // Source files are read lazily by diagnostics and sourcemap queries,
    // so they must outlive the database returned from this function.
    let _ = fixture_dir.keep();
    db
}

/// Creates a [`DbImpl`](crate::codegen::db::DbImpl) containing rustdoc inputs
/// for inline Rust code.
///
/// The macro is only available to unit tests. Configure API inputs separately
/// with [`Api::set`](crate::codegen::api::Api::set). Place `@file: "path";`
/// before an inline module to emit its body into that file and replace the
/// module with a `#[path = "path"]` declaration.
///
/// ```no_run
/// use rust_curse::test_db;
///
/// let db = test_db! {
///     @file: "models/user.rs";
///     pub mod user {
///         pub struct User;
///     }
/// };
/// # let _ = db;
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
        let src = $crate::rust_source!($($source)*);
        let files = [$(($path, $crate::rust_source!($($body)*))),*];
        $crate::codegen::test::build_test_db(src, &files)
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

/// Builds the Markdown body and destination path for a generated-code snapshot.
#[cfg(test)]
pub fn build_snapshot(
    source: &str,
    generated: &str,
    output_language: &str,
    source_file: &str,
) -> (String, std::path::PathBuf) {
    let wrapped = format!("fn snapshot() {{\n{source}\n}}\n");
    let formatted = prettyplease::unparse(
        &syn::parse_file(&wrapped).expect("snapshot source should be valid Rust"),
    );

    let source = formatted
        .strip_prefix("fn snapshot() {\n")
        .and_then(|body| body.strip_suffix("}\n"))
        .expect("prettyplease should preserve the snapshot function wrapper")
        .lines()
        .map(|line| line.strip_prefix("    ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");

    let thread = std::thread::current();
    let test_name = thread.name().unwrap_or("<unknown>");
    let body = indoc::formatdoc! {r#"
        ## Test

        ```text
        {test_name}
        ```

        ## Source

        ```rs
        {source}
        ```

        ## Output

        ```{output_language}
        {generated}
        ```
    "#};
    let snapshot_category = std::path::Path::new(source_file)
        .file_stem()
        .expect("snapshot source should have a file name");
    let snapshot_path = std::path::Path::new("_snapshots").join(snapshot_category);

    (body, snapshot_path)
}

/// Asserts a Markdown snapshot containing the test name, formatted source, and
/// generated output.
///
/// Prefix a block with `@lang: "language";` to set the output code block's
/// language. Output defaults to Rust when the attribute is omitted.
#[cfg(test)]
#[macro_export]
macro_rules! assert_snap {
    (@assert $language:expr, $source:expr, $expression:expr) => {{
        let generated = $expression;
        let (body, snapshot_path) = $crate::codegen::test::build_snapshot(
            $source,
            &generated,
            $language,
            file!(),
        );

        ::insta::with_settings!({
            snapshot_path => snapshot_path,
            prepend_module_to_snapshot => false,
        }, {
            ::insta::assert_snapshot!(body);
        });
    }};
    ({ @lang: $language:literal; $($body:tt)* }) => {
        $crate::codegen::test::assert_snap!(
            @assert $language,
            ::std::stringify!($($body)*),
            { $($body)* }
        )
    };
    ({$($body:tt)*}) => {
        $crate::codegen::test::assert_snap!(
            @assert "rs",
            ::std::stringify!($($body)*),
            { $($body)* }
        )
    };
    ($expression:expr) => {
        $crate::codegen::test::assert_snap!(
            @assert "rs",
            ::std::stringify!($expression),
            $expression
        )
    };
}

#[cfg(test)]
pub use crate::assert_snap;
