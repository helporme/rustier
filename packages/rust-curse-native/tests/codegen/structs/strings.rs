use crate::utils::*;

#[test]
fn plain_struct_with_string() {
    assert_snap! {
        @gen: ir, ts;
        @category: strings;

        pub struct PlainStruct {
            pub value: String,
        }
    }
}

#[test]
fn tuple_struct_with_string() {
    assert_snap! {
        @gen: ir, ts;
        @category: strings;

        pub struct TupleStruct(pub String);
    }
}

#[test]
fn plain_struct_with_qualified_string() {
    assert_snap! {
        @gen: ir, ts;
        @category: strings;

        pub struct PlainStruct {
            pub value: std::string::String,
        }
    }
}

#[test]
fn tuple_struct_with_qualified_string() {
    assert_snap! {
        @gen: ir, ts;
        @category: strings;

        pub struct TupleStruct(pub std::string::String);
    }
}
