//! Tests for module documentation generation.

use crate::utils::*;

#[test]
fn root_docs() {
    assert_snap! {
        @generate: ir;
        @category: docs;

        //! Root module docs.

        pub fn answer() -> i32 {
            42
        }
    }
}

#[test]
fn inner_mod_docs() {
    assert_snap! {
        @generate: ir, ts;
        @category: docs;

        /// (^.^)
        pub mod math {
            pub fn answer() -> i32 {
                42
            }
        }
    }
}
