use crate::utils::*;

#[test]
fn fn_accepting_structs() {
    assert_snap! {
        @generate: ts, ir, node, wasm;
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
        @generate: ts, ir, node, wasm;
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
