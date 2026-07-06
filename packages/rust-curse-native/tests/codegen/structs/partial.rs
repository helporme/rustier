//! Tests for partially lowered structs with skipped fields.

use crate::utils::*;

#[test]
fn plain_struct_with_unsupported_field() {
    assert_snap! {
        @gen: ir;
        @category: partial;

        pub struct PlainStruct {
            pub supported: i32,
            pub unsupported: u64,
        }
    }
}

#[test]
fn tuple_struct_with_unsupported_field() {
    assert_snap! {
        @gen: ir;
        @category: partial;

        pub struct TupleStruct(pub i32, pub u64, pub bool);
    }
}

#[test]
fn plain_struct_with_private_and_unsupported_fields() {
    assert_snap! {
        @gen: ir;
        @category: partial;

        pub struct PlainStruct {
            pub supported: i32,
            private: bool,
            pub unsupported: u64,
        }
    }
}

#[test]
fn tuple_struct_with_private_and_unsupported_fields() {
    assert_snap! {
        @gen: ir;
        @category: partial;

        pub struct TupleStruct(pub i32, bool, pub u64, pub bool);
    }
}
