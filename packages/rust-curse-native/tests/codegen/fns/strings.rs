use crate::utils::*;

#[test]
fn fn_accepting_ref_str() {
    assert_snap! {
        @generate: ts, ir, node, wasm;
        @category: strings;

        pub fn foo(_: &str) { }
    }
}

#[test]
fn fn_accepting_mut_str() {
    assert_snap! {
        @generate: ts, ir, node, wasm;
        @category: strings;

        pub fn foo(_: &str) { }
    }
}

#[test]
fn fn_accepting_string() {
    assert_snap! {
        @generate: ts, ir, node, wasm;
        @category: strings;

        pub fn foo(_: String) { }
    }
}

#[test]
fn fn_accepting_ref_string() {
    assert_snap! {
        @generate: ts, ir, node, wasm;
        @category: strings;

        pub fn foo(_: &String) { }
    }
}

#[test]
fn fn_accepting_mut_string() {
    assert_snap! {
        @generate: ts, ir, node, wasm;
        @category: strings;

        pub fn foo(_: &mut String) { }
    }
}

#[test]
fn fn_accepting_qualified_string() {
    assert_snap! {
        @generate: ts, ir, node, wasm;
        @category: strings;

        pub fn foo(_: std::string::String) { }
    }
}

#[test]
fn fn_returning_string() {
    assert_snap! {
        @generate: ts, ir, node, wasm;
        @category: strings;

        pub fn foo() -> String {
            String::from("Hi")
        }
    }
}
