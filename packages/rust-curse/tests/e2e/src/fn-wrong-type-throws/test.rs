#[derive(Clone)]
pub struct ValueStruct {
    pub value: i32,
}

#[derive(Clone)]
pub struct ChildStruct {
    pub value: i32,
}

pub fn identity_i32(value: i32) -> i32 {
    value
}

pub fn value_struct_value(value: &ValueStruct) -> i32 {
    value.value
}

pub fn make_child_struct(value: i32) -> ChildStruct {
    ChildStruct { value }
}
