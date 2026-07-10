use crate::utils::*;

#[test]
fn plain_struct_with_struct_fields() {
    assert_snap! {
        @gen: ir, ts, node, wasm;
        @category: structs;

        #[derive(Clone)]
        pub struct PlainField {
            pub value: i32,
        }

        #[derive(Clone)]
        pub struct TupleField(pub String);

        #[derive(Clone)]
        pub struct Parent {
            pub plain: PlainField,
            pub tuple: TupleField,
        }
    }
}

#[test]
fn tuple_struct_with_struct_fields() {
    assert_snap! {
        @gen: ir, ts, node, wasm;
        @category: structs;

        #[derive(Clone)]
        pub struct PlainField {
            pub value: i32,
        }

        #[derive(Clone)]
        pub struct TupleField(pub String);

        #[derive(Clone)]
        pub struct Parent(pub PlainField, pub TupleField);
    }
}
