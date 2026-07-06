//! Tests for simple struct generation with all primitive types as fields.

use crate::utils::*;

#[test]
fn plain_struct_with_as_is_primitives() {
    assert_snap! {
        @gen: ir;
        @category: primitives;

        pub struct PlainStruct {
            pub a: bool,
            pub d: i8,
            pub e: i16,
            pub f: i32,
            pub g: u8,
            pub h: u16,
            pub i: u32,
            pub k: f64,
        }
    }
}

#[test]
fn tuple_struct_with_as_is_primitives() {
    assert_snap! {
        @gen: ir;
        @category: primitives;

        pub struct TupleStruct(pub bool, pub i8, pub i16, pub i32, pub u8, pub u16, pub u32, pub f64);
    }
}

#[test]
fn plain_struct_with_char() {
    assert_snap! {
        @gen: ir;
        @category: primitives;

        pub struct PlainStruct {
            pub value: char,
        }
    }
}

#[test]
fn tuple_struct_with_char() {
    assert_snap! {
        @gen: ir;
        @category: primitives;

        pub struct TupleStruct(pub char);
    }
}

#[test]
fn plain_struct_with_f32() {
    assert_snap! {
        @gen: ir;
        @category: primitives;

        pub struct PlainStruct {
            pub value: f32,
        }
    }
}

#[test]
fn tuple_struct_with_f32() {
    assert_snap! {
        @gen: ir;
        @category: primitives;

        pub struct TupleStruct(pub f32);
    }
}
