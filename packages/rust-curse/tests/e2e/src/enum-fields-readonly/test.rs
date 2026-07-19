#[derive(Clone)]
pub struct ChildStruct {
    pub value: i32,
}

pub struct NonCloneChildStruct {
    pub value: i32,
}

/// Returns a cloneable child struct.
pub fn make_child_struct(value: i32) -> ChildStruct {
    ChildStruct { value }
}

/// Mutates a non-cloneable child through a mutable reference.
pub fn set_non_clone_child_value(child: &mut NonCloneChildStruct, value: i32) {
    child.value = value;
}

/// An enum whose common field is writable on one variant and readonly on another.
pub enum MixedReadonlyEnum {
    Writable { item: ChildStruct },
    Readonly { item: NonCloneChildStruct },
}

/// Returns the writable variant containing a cloneable child.
pub fn make_writable_mixed_readonly_enum(value: i32) -> MixedReadonlyEnum {
    MixedReadonlyEnum::Writable {
        item: ChildStruct { value },
    }
}

/// Returns the readonly variant containing a non-cloneable child.
pub fn make_readonly_mixed_readonly_enum(value: i32) -> MixedReadonlyEnum {
    MixedReadonlyEnum::Readonly {
        item: NonCloneChildStruct { value },
    }
}
