//! Tests for function documentation generation.

use crate::utils::*;

#[test]
fn docs() {
    assert_snap! {
        @gen: ir, ts;

        /// Adds two numbers.
        pub fn add(a: i32, b: i32) -> i32 {
            a + b
        }
    }
}
