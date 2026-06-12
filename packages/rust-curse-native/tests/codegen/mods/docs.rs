//! Tests for module documentation generation.

use crate::utils::*;

#[test]
fn docs() {
    assert_snap! {
        @gen: ir, ts;

        /// (^.^)
        pub mod math {
            pub fn answer() -> i32 {
                42
            }
        }
    }
}
