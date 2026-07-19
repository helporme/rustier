/// A primitive-valued child structure.
#[derive(Clone)]
pub struct ChildStruct {
    pub value: i32,
}

/// A parent containing a structure-valued field.
#[derive(Clone)]
pub struct ParentStruct {
    pub child: ChildStruct,
}

/// Creates a child with the supplied value.
pub fn make_child_struct(value: i32) -> ChildStruct {
    ChildStruct { value }
}

/// Creates a parent whose child has the supplied value.
pub fn make_parent_struct(value: i32) -> ParentStruct {
    ParentStruct {
        child: ChildStruct { value },
    }
}
