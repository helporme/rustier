//! Tests for struct documentation lowering.

use crate::utils::*;

#[test]
fn unit_struct_docs() {
    assert_snap! {
        @gen: ir, ts;
        @category: docs;

        /// Unit struct docs.
        pub struct UnitStruct;
    }
}

#[test]
fn plain_struct_and_field_docs() {
    assert_snap! {
        @gen: ir, ts;
        @category: docs;

        /// Plain struct docs.
        pub struct PlainStruct {
            /// Plain field docs.
            pub value: i32,
        }
    }
}

#[test]
fn tuple_struct_and_field_docs() {
    assert_snap! {
        @gen: ir, ts;
        @category: docs;

        /// Tuple struct docs.
        pub struct TupleStruct(
            /// Tuple field docs.
            pub i32,
        );
    }
}
