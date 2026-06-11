//! Tests for module documentation generation.

use crate::utils::*;

#[test]
pub fn docs() {
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
