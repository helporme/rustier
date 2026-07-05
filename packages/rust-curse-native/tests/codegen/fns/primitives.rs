//! Tests for simple function generation with all primitive types as arguments
//! and return values.

use crate::utils::*;

#[test]
fn fn_with_all_as_is_primitives() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: primitives;

        pub fn foo(
            a: bool,
            d: i8,
            e: i16,
            f: i32,
            g: u8,
            h: u16,
            i: u32,
            k: f64,
        ) { }
    }
}
#[test]
fn fn_with_all_ref_as_is_primitives() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: primitives;

        pub fn foo(
            a: &bool,
            d: &i8,
            e: &i16,
            f: &i32,
            g: &u8,
            h: &u16,
            i: &u32,
            k: &f64,
        ) { }
    }
}

#[test]
fn fn_with_all_mut_ref_as_is_primitives() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: primitives;

        pub fn foo(
            a: &mut bool,
            d: &mut i8,
            e: &mut i16,
            f: &mut i32,
            g: &mut u8,
            h: &mut u16,
            i: &mut u32,
            k: &mut f64,
        ) { }
    }
}

#[test]
fn fn_returning_as_is_primitive() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: primitives;

        pub fn foo() -> bool {
            true
        }
    }
}

#[test]
fn fn_no_args_no_return() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: primitives;

        pub fn foo() { }
    }
}

#[test]
fn fn_accepting_char() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: primitives;

        pub fn foo(value: char) { }
    }
}

#[test]
fn fn_returning_char() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: primitives;

        pub fn foo() -> char {
            '🦀'
        }
    }
}

#[test]
fn fn_accepting_f32() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: primitives;

        pub fn foo(value: f32) { }
    }
}

#[test]
fn fn_returning_f32() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: primitives;

        pub fn foo() -> f32 {
            1.25
        }
    }
}
