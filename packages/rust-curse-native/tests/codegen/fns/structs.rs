use crate::utils::*;

#[test]
fn fn_accepting_structs() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: structs;

        #[derive(Clone)]
        pub struct PlainStruct {
            pub value: i32,
        }

        #[derive(Clone)]
        pub struct TupleStruct(pub String);

        pub fn foo(
            moved: PlainStruct,
            borrowed: &TupleStruct,
            mutable: &mut PlainStruct,
        ) {
            let _ = (moved, borrowed, mutable);
        }
    }
}

#[test]
fn fn_returning_struct() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: structs;

        #[derive(Clone)]
        pub struct PlainStruct {
            pub value: i32,
        }

        pub fn foo() -> PlainStruct {
            PlainStruct { value: 42 }
        }
    }
}

#[test]
fn fn_with_mixed_parameters() {
    assert_snap! {
        @gen: ts, ir, node, wasm;
        @category: structs;

        #[derive(Clone)]
        pub struct PlainStruct {
            pub value: i32,
        }

        #[derive(Clone)]
        pub struct TupleStruct(pub String);

        pub fn structs_first(
            moved_plain: PlainStruct,
            moved_tuple: TupleStruct,
            mutable: &mut PlainStruct,
            text: String,
            borrowed: &TupleStruct,
            number: i32,
        ) -> TupleStruct {
            let _ = (
                moved_plain,
                moved_tuple,
                borrowed,
                mutable,
                text,
                number,
            );
            todo!()
        }

        pub fn values_first(
            text: &str,
            number: f64,
            moved: TupleStruct,
            mutable: &mut PlainStruct,
            borrowed: &PlainStruct,
        ) -> PlainStruct {
            let _ = (text, number, moved, mutable, borrowed);
            todo!()
        }

        pub fn interleaved(
            number: u32,
            moved: PlainStruct,
            text: String,
            borrowed: &TupleStruct,
            flag: bool,
            mutable: &mut PlainStruct,
        ) -> TupleStruct {
            let _ = (number, moved, text, borrowed, flag, mutable);
            todo!()
        }
    }
}
