use crate::utils::*;

#[test]
fn fn_accepting_ref_str() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: strings;

        pub fn foo(_: &str) { }
    }
}

#[test]
fn fn_accepting_mut_str() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: strings;

        pub fn foo(_: &str) { }
    }
}
