//! Tests for function generation inside Rust modules.

use crate::utils::*;

#[test]
fn two_mods() {
    assert_snap! {
        @generate: ir, ts;

        pub mod a {
            pub fn a_fn() {}
        }

        pub mod b {
            pub fn b_fn() {}
        }
    }
}

#[test]
fn submod() {
    assert_snap! {
        @generate: ir, ts;

        pub mod a {
            pub fn a_fn() {}

            pub mod b {
                pub fn b_fn() {}
            }
        }
    }
}
