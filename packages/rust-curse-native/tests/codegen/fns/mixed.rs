use crate::utils::*;

#[test]
fn structs_first_mixed_parameters() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: mixed;

        #[derive(Clone)]
        pub struct PlainStruct {
            pub value: i32,
        }

        #[derive(Clone)]
        pub struct TupleStruct(pub String);

        pub fn foo(
            moved_plain: PlainStruct,
            moved_tuple: TupleStruct,
            mutable: &mut PlainStruct,
            text: String,
            borrowed_text: &String,
            mutable_text: &mut String,
            borrowed: &TupleStruct,
            number: i32,
        ) -> TupleStruct {
            let _ = (
                moved_plain,
                moved_tuple,
                borrowed,
                mutable,
                text,
                borrowed_text,
                mutable_text,
                number,
            );
            todo!()
        }
    }
}

#[test]
fn values_first_mixed_parameters() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: mixed;

        #[derive(Clone)]
        pub struct PlainStruct {
            pub value: i32,
        }

        #[derive(Clone)]
        pub struct TupleStruct(pub String);

        pub fn foo(
            text: &str,
            borrowed_text: &String,
            mutable_text: &mut String,
            number: f64,
            moved: TupleStruct,
            mutable: &mut PlainStruct,
            borrowed: &PlainStruct,
        ) -> PlainStruct {
            let _ = (
                text,
                borrowed_text,
                mutable_text,
                number,
                moved,
                mutable,
                borrowed,
            );
            todo!()
        }
    }
}

#[test]
fn interleaved_mixed_parameters() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: mixed;

        #[derive(Clone)]
        pub struct PlainStruct {
            pub value: i32,
        }

        #[derive(Clone)]
        pub struct TupleStruct(pub String);

        pub fn foo(
            number: u32,
            moved: PlainStruct,
            text: String,
            borrowed: &TupleStruct,
            borrowed_text: &String,
            flag: bool,
            mutable_text: &mut String,
            mutable: &mut PlainStruct,
        ) -> TupleStruct {
            let _ = (
                number,
                moved,
                text,
                borrowed,
                borrowed_text,
                flag,
                mutable_text,
                mutable,
            );
            todo!()
        }
    }
}
