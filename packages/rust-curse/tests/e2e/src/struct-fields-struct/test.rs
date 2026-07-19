/// A clonable nested child value.
#[derive(Clone)]
pub struct ChildStruct {
    pub value: i32,
}

/// A clonable parent containing a child value.
#[derive(Clone)]
pub struct ParentStruct {
    pub child: ChildStruct,
}

/// A clonable grandparent containing a parent value.
#[derive(Clone)]
pub struct GrandparentStruct {
    pub parent: ParentStruct,
}

/// A nested child that cannot be cloned.
pub struct NonCloneChildStruct {
    pub value: i32,
}

/// A parent containing a non-clonable child.
pub struct NonCloneParentStruct {
    pub child: NonCloneChildStruct,
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

/// Creates a grandparent with the supplied deeply nested value.
pub fn make_grandparent_struct(value: i32) -> GrandparentStruct {
    GrandparentStruct {
        parent: ParentStruct {
            child: ChildStruct { value },
        },
    }
}

/// Creates a parent containing a non-clonable child with the supplied value.
pub fn make_non_clone_parent_struct(value: i32) -> NonCloneParentStruct {
    NonCloneParentStruct {
        child: NonCloneChildStruct { value },
    }
}

/// Reads a child value through a shared reference.
pub fn child_value(child: &ChildStruct) -> i32 {
    child.value
}

/// Sets a child value through a mutable reference.
pub fn set_child_value(child: &mut ChildStruct, value: i32) {
    child.value = value;
}

/// Copies a child value into a mutably borrowed parent's child.
pub fn copy_child_to_parent_child(source: &ChildStruct, parent: &mut ParentStruct) {
    parent.child.value = source.value;
}

/// Reads a nested child value through a shared parent reference.
pub fn parent_child_value(parent: &ParentStruct) -> i32 {
    parent.child.value
}

/// Mutates an owned child and returns the independent value.
pub fn mutate_owned_child(mut child: ChildStruct, new_value: i32) -> ChildStruct {
    child.value = new_value;
    child
}

/// Reads a non-clonable child through a shared reference.
pub fn non_clone_child_value(child: &NonCloneChildStruct) -> i32 {
    child.value
}

/// Sets a non-clonable child through a mutable reference.
pub fn set_non_clone_child_value(child: &mut NonCloneChildStruct, value: i32) {
    child.value = value;
}
