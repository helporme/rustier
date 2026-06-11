use crate::utils::*;

fn assert_skipped_fn(diags: &[rust_curse::diag::Diag], expected_causes: &[&str]) {
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "rust-curse::skipped_fn");

    let actual_causes = diags[0]
        .causes
        .iter()
        .map(|diag| diag.code.as_str())
        .collect::<Vec<_>>();
    assert_eq!(actual_causes, expected_causes);
}

#[test]
pub fn unsupported_async_fn() {
    let (diags, _) = diag! {
        pub async fn foo() { todo!() }
    };

    assert_skipped_fn(&diags, &["rust-curse::unsupported_async_fn"]);
}

#[test]
pub fn unsupported_unsafe_fn() {
    let (diags, _) = diag! {
        pub unsafe fn foo() { todo!() }
    };

    assert_skipped_fn(&diags, &["rust-curse::unsupported_unsafe_fn"]);
}

#[test]
pub fn unsupported_abi_fn() {
    let (diags, _) = diag! {
        pub extern "C" fn foo() { todo!() }
    };

    assert_skipped_fn(&diags, &["rust-curse::unsupported_abi"]);
}

#[test]
pub fn unsupported_variadic_fn() {
    let (diags, _) = diag! {
        #![feature(c_variadic)]

        pub unsafe extern "C" fn foo(_: i32, _: ...) { todo!() }
    };

    assert_skipped_fn(
        &diags,
        &[
            "rust-curse::unsupported_unsafe_fn",
            "rust-curse::unsupported_abi",
            "rust-curse::unsupported_variadic_fn",
        ],
    );
}

#[test]
pub fn unsupported_generic_fn() {
    let (diags, _) = diag! {
        pub fn foo<T>(_: T) { todo!() }
    };

    assert_skipped_fn(
        &diags,
        &[
            "rust-curse::unsupported_generic_fn",
            "rust-curse::unsupported_param",
        ],
    );
}

#[test]
pub fn unsupported_param() {
    let (diags, _) = diag! {
        pub fn foo(_: u64) { todo!() }
    };

    assert_skipped_fn(&diags, &["rust-curse::unsupported_param"]);
    assert_eq!(diags[0].causes[0].causes.len(), 1);
    assert_eq!(
        diags[0].causes[0].causes[0].code,
        "rust-curse::unsupported_primitive"
    );
}

#[test]
pub fn borrowed_return() {
    let (diags, _) = diag! {
        pub fn foo(value: &i32) -> &i32 { value }
    };

    assert_skipped_fn(&diags, &["rust-curse::borrowed_return"]);
}

#[test]
pub fn primitive_mut_ref_ignored() {
    let (diags, _) = diag! {
        pub fn foo(value: &mut i32) { *value += 1; }
    };

    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "rust-curse::primitive_mut_ref_ignored");
    assert!(diags[0].causes.is_empty());
}

#[test]
pub fn snap_chained_diags() {
    assert_snap! {
        @gen: diags;

        pub struct MyStruct {
            pub field: i32,
        }

        pub unsafe fn foo(a: MyStruct, b: u64, c: &&mut usize) { }
    }
}
