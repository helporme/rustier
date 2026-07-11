use crate::utils::*;

#[test]
fn non_clonable_struct_field_is_readonly() {
    assert_snap! {
        @gen: node, wasm;
        @category: "incremental_regressions";

        pub struct Child {
            pub value: i32,
        }

        pub struct Parent {
            pub child: Child,
        }

        pub fn make_parent(value: i32) -> Parent {
            Parent {
                child: Child { value },
            }
        }

        pub fn child_value(child: &Child) -> i32 {
            child.value
        }

        pub fn set_child_value(child: &mut Child, value: i32) {
            child.value = value;
        }
    }
}
