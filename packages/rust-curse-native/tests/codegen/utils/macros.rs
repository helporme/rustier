//! Test fixture macros for rustdoc, IR, diagnostics, and snapshots.

#![allow(unused_imports, unused_macros)]

macro_rules! rustdoc {
    ($($tt:tt)*) => {{
        $crate::utils::rustdoc::rustdoc_from_src(::std::stringify!($($tt)*)).crate_
    }};
}
pub(crate) use rustdoc;

macro_rules! ir {
    ($($tt:tt)*) => {{
        let fixture = $crate::utils::rustdoc::rustdoc_from_src(::std::stringify!($($tt)*));
        let db: &'static ::rust_curse::codegen::db::DbImpl =
            ::std::boxed::Box::leak(::std::boxed::Box::new(
                $crate::utils::rustdoc::db_with_rustdoc(fixture.crate_.clone()),
            ));
        let root_id = $crate::utils::rustdoc::root_id(db, &fixture.crate_);
        let mod_ = ::rust_curse::codegen::rustdoc::lower_mod(db, root_id);
        (mod_, db)
    }};
}
pub(crate) use ir;

macro_rules! diag {
    ($($tt:tt)*) => {{
        let fixture = $crate::utils::rustdoc::rustdoc_from_src(::std::stringify!($($tt)*));
        let db: &'static ::rust_curse::codegen::db::DbImpl =
            ::std::boxed::Box::leak(::std::boxed::Box::new(
                $crate::utils::rustdoc::db_with_rustdoc(fixture.crate_.clone()),
            ));
        let root_id = $crate::utils::rustdoc::root_id(db, &fixture.crate_);
        let diags = $crate::utils::diags::collect_diags_mod(db, root_id);
        (diags, db)
    }};
}
pub(crate) use diag;

macro_rules! assert_snap {
    (@gen: [$($category:ident),+ $(,)?]; @snapshot_path: $snapshot_path:expr; $($tt:tt)*) => {{
        let categories = &[$(::std::stringify!($category)),+];
        let body = $crate::utils::snap::build_snap(::std::stringify!($($tt)*), categories);
        ::insta::with_settings!({ snapshot_path => $snapshot_path, prepend_module_to_snapshot => false }, {
            ::insta::assert_snapshot!(body);
        });
    }};
    (@gen: $($category:ident),+ $(,)?; @category: $snapshot_category:ident; $($tt:tt)*) => {{
        $crate::utils::assert_snap!(
            @gen: [$($category),+];
            @snapshot_path: ::std::concat!("snapshots/", ::std::stringify!($snapshot_category));
            $($tt)*
        )
    }};
    (@gen: $($category:ident),+ $(,)?; $($tt:tt)*) => {{
        $crate::utils::assert_snap!(
            @gen: [$($category),+];
            @snapshot_path: "snapshots";
            $($tt)*
        )
    }};
    ({ [$($category:ident),+ $(,)?] { $($tt:tt)* } }) => {
        $crate::utils::assert_snap!(@gen: $($category),+; $($tt)*)
    };
    ([$($category:ident),+ $(,)?] { $($tt:tt)* }) => {
        $crate::utils::assert_snap!(@gen: $($category),+; $($tt)*)
    };
}
pub(crate) use assert_snap;
