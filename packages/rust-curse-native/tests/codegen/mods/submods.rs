//! Tests for function generation inside Rust modules.

use crate::utils::*;

#[test]
pub fn two_mods() {
    assert_snap! {
        @gen: ir, ts;

        pub mod a {
            pub fn a_fn() {}
        }

        pub mod b {
            pub fn b_fn() {}
        }
    }
}

#[test]
pub fn submod() {
    assert_snap! {
        @gen: ir, ts;

        pub mod a {
            pub fn a_fn() {}

            pub mod b {
                pub fn b_fn() {}
            }
        }
    }
}
