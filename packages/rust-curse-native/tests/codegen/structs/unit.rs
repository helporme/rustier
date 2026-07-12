use crate::utils::*;

#[test]
fn plain_unit_struct() {
    assert_snap! {
        @generate: ir, ts, wasm;
        @category: unit;

        pub struct PlainStruct {}
    }
}

#[test]
fn tuple_unit_struct() {
    assert_snap! {
        @generate: ir, ts, wasm, node;
        @category: unit;

        pub struct TupleStruct();
    }
}

#[test]
fn unit_struct() {
    assert_snap! {
        @generate: ir, ts, wasm;
        @category: unit;

        pub struct UnitStruct;
    }
}
