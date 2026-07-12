use crate::utils::*;

#[test]
fn plain_struct_with_some_pub_fields() {
    assert_snap! {
        @generate: ir, ts, wasm, node;
        @category: visibility;

        pub struct PlainStruct {
            pub a: bool,
            b: bool,
            pub c: bool,
        }
    }
}

#[test]
fn plain_struct_with_none_pub_fields() {
    assert_snap! {
        @generate: ir, ts, wasm, node;
        @category: visibility;

        pub struct PlainStruct {
            a: bool,
            b: bool,
            c: bool,
        }
    }
}

#[test]
fn tuple_struct_with_some_pub_fields() {
    assert_snap! {
        @generate: ir, ts, wasm, node;
        @category: visibility;

        pub struct TupleStruct(pub bool, bool, pub bool);
    }
}

#[test]
fn tuple_struct_with_none_pub_fields() {
    assert_snap! {
        @generate: ir, ts, wasm, node;
        @category: visibility;

        pub struct TupleStruct(bool, bool, bool);
    }
}
