/// A structure containing a primitive field.
#[derive(Clone)]
pub struct ChildStruct {
    pub value: i32,
}

/// A tuple structure with only its second field exposed.
#[allow(dead_code)]
#[derive(Clone)]
pub struct TupleStruct(i32, pub i32);

/// Creates a child with the supplied value.
pub fn make_child_struct(value: i32) -> ChildStruct {
    ChildStruct { value }
}

/// Reads a child value through a shared reference.
pub fn child_value(child: &ChildStruct) -> i32 {
    child.value
}

/// Creates a sparse tuple structure with the supplied exposed value.
pub fn make_tuple_struct(value: i32) -> TupleStruct {
    TupleStruct(0, value)
}
