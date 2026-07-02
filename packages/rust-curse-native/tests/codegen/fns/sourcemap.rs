//! Tests for function TypeScript source map generation.

use crate::utils::*;

#[test]
fn sourcemap() {
    assert_snap! {
        @gen: ts, ts_sourcemap;

        pub fn add(a: i32, b: i32) -> i32 {
            a + b
        }

        pub fn greet(name: &str) {
            let _ = name;
        }
    }
}
