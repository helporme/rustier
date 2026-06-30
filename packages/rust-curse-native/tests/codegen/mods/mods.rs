use crate::utils::*;

#[test]
fn js_glue_code() {
    assert_snap! {
        @gen: js, wasm_web_js;

        pub fn a_fn() {}

        pub fn b_fn() {}
    }
}
