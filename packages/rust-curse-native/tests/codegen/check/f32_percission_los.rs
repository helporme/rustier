use crate::utils::*;

fn assert_f32_precision_loss(diags: &[rust_curse::codegen::diag::Diag]) {
    diags::assert_single_diag_without_related(diags, "rust_curse::f32_precision_loss");
}

#[test]
fn struct_field() {
    let (diags, _) = diag! {
        pub struct Foo {
            pub value: f32,
        }
    };

    assert_f32_precision_loss(&diags);
}

#[test]
fn fn_return_value() {
    let (diags, _) = diag! {
        pub fn foo() -> f32 {
            1.25
        }
    };

    assert_f32_precision_loss(&diags);
}

#[test]
fn fn_argument() {
    let (diags, _) = diag! {
        pub fn foo(value: f32) {
            let _ = value;
        }
    };

    assert_f32_precision_loss(&diags);
}

#[test]
fn fn_return_value_and_argument() {
    let (diags, _) = diag! {
        pub fn foo(value: f32) -> f32 {
            value
        }
    };

    assert_f32_precision_loss(&diags);
}
