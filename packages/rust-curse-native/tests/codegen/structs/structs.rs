use crate::utils::*;

#[test]
fn plain_struct_with_struct_fields() {
    assert_snap! {
        @generate: ir, ts, node, wasm;
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
        @generate: ir, ts, node, wasm;
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

#[test]
fn plain_struct_with_non_clonable_struct_field() {
    assert_snap! {
        @generate: ir, ts, node, wasm;
        @category: structs;

        pub struct Field;

        pub struct Parent {
            pub field: Field,
        }
    }
}

#[test]
fn tuple_struct_with_non_clonable_struct_field() {
    assert_snap! {
        @generate: ir, ts, node, wasm;
        @category: structs;

        pub struct Field;

        pub struct Parent(pub Field);
    }
}

#[test]
fn plain_struct_with_mixed_clonable_struct_fields() {
    assert_snap! {
        @generate: ir, ts, node, wasm;
        @category: structs;

        #[derive(Clone)]
        pub struct ClonableField;

        pub struct NonClonableField;

        pub struct Parent {
            pub clonable: ClonableField,
            pub non_clonable: NonClonableField,
        }
    }
}

#[test]
fn tuple_struct_with_mixed_clonable_struct_fields() {
    assert_snap! {
        @generate: ir, ts, node, wasm;
        @category: structs;

        #[derive(Clone)]
        pub struct ClonableField;

        pub struct NonClonableField;

        pub struct Parent(pub ClonableField, pub NonClonableField);
    }
}
