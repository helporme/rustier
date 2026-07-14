//! Test-only helpers for constructing fully initialized codegen databases.

// This intentionally mirrors `tests/codegen/utils/rustdoc.rs` since It's easier
// to copy 100 lines than establish some feature flags to allow this code be
// available from integration tests.

use crate::codegen::{db::DbImpl, ir::IrId, rustdoc::RustdocId};
pub use crate::test_db;

#[doc(hidden)]
pub const TEST_CRATE_NAME: &str = "rustdoc_test_crate";

impl DbImpl {
    /// Returns the IR ID for an item at a path relative to the test crate root.
    ///
    /// An empty path returns the test crate's root ID.
    ///
    /// ```no_run
    /// use rust_curse::test_db;
    ///
    /// let db = test_db! {
    ///     @api: node;
    /// };
    /// let root = db.ir_id("");
    /// assert_eq!(root.path(&db), &["rustdoc_test_crate"]);
    /// ```
    pub fn ir_id(&self, path: &str) -> IrId<'_> {
        let mut segments = vec![TEST_CRATE_NAME.to_string()];
        if !path.is_empty() {
            segments.extend(path.split("::").map(str::to_string));
        }

        IrId::new(self, segments)
    }

    /// Returns the rustdoc ID for an item at `path`, panicking if it does not
    /// exist.
    ///
    /// ```no_run
    /// use rust_curse::test_db;
    ///
    /// let db = test_db! {
    ///     @api: node;
    ///
    ///     pub mod path {
    ///         pub fn item() {}
    ///     }
    /// };
    /// let id = db.rustdoc_id("path::item");
    /// assert_eq!(id.item(&db).name.as_deref(), Some("item"));
    /// ```
    pub fn rustdoc_id(&self, path: &str) -> RustdocId<'_> {
        self.ir_id(path).rustdoc_id(self)
    }
}

/// Creates a [`DbImpl`](crate::codegen::db::DbImpl) containing rustdoc and API
/// inputs for inline Rust code.
///
/// `wasm` selects the wasm-bindgen web target, while `node` selects napi. The
/// macro can only be invoked from test targets, including doctests.
///
/// ```no_run
/// use rust_curse::test_db;
///
/// let db = test_db! {
///     @api: wasm;
///
///     pub fn get_string() -> String {
///         todo!()
///     }
/// };
/// # let _ = db;
/// ```
#[macro_export]
macro_rules! test_db {
    (@api: wasm; $($source:tt)*) => {
        $crate::test_db! {
            @build: $crate::codegen::api::ApiKind::Wasm {
                target: $crate::codegen::api::WasmTarget::Web,
            };
            $($source)*
        }
    };
    (@api: node; $($source:tt)*) => {
        $crate::test_db! {
            @build: $crate::codegen::api::ApiKind::Node;
            $($source)*
        }
    };
    (@build: $api:expr_2021; $($source:tt)*) => {{
        const _: () = assert!(
            cfg!(test) || ::core::option_env!("UNSTABLE_RUSTDOC_TEST_PATH").is_some(),
            "`test_db!` is only available in tests and doctests",
        );
        const TEST_CRATE_NAME: &str = $crate::codegen::test::TEST_CRATE_NAME;

            let raw_src = ::std::stringify!($($source)*);
            let src = match ::syn::parse_str::<::syn::File>(raw_src) {
                Ok(file) => ::prettyplease::unparse(&file),
                Err(_) => raw_src.to_string(),
            };
            let fixture_dir = ::tempfile::tempdir().expect("create test crate tempdir");
            let crate_dir = fixture_dir.path().to_path_buf();
            let src_dir = crate_dir.join("src");

            ::std::fs::create_dir_all(&src_dir).expect("create test crate src directory");
            ::std::fs::write(
                crate_dir.join("Cargo.toml"),
                ::std::format!(
                    "[package]\n\
                     name = \"{TEST_CRATE_NAME}\"\n\
                     version = \"0.0.0\"\n\
                     edition = \"2021\"\n\
                     publish = false\n\
                     \n\
                     [lib]\n\
                     path = \"src/lib.rs\"\n\
                     \n\
                     [workspace]\n"
                ),
            )
            .expect("write test crate manifest");
            ::std::fs::write(src_dir.join("lib.rs"), src).expect("write test crate source");

            let crate_body = $crate::tools::rustdoc::run(&crate_dir.join("Cargo.toml"))
                .unwrap_or_else(|err| panic!("rustdoc generation failed: {err}"));
            let crates = ::std::collections::BTreeMap::from([(
                TEST_CRATE_NAME.to_string(),
                $crate::codegen::rustdoc::Crate {
                    body: crate_body,
                    is_external: false,
                    dir: crate_dir,
                },
            )]);

            let mut db = $crate::codegen::db::DbImpl::default();
            $crate::codegen::rustdoc::RustdocCrates::set(&mut db, crates);
            $crate::codegen::api::Api::set(&mut db, $api);

            // Source files are read lazily by diagnostics and sourcemap queries,
            // so they must outlive the database returned from this expression.
        let _ = fixture_dir.keep();
        db
    }};
}
