#[derive(Clone)]
pub struct ChildStruct {
    pub value: i32,
}

#[derive(Clone)]
pub enum BoundaryEnum {
    First {
        unique: i32,
        shared: i32,
        child: ChildStruct,
    },
    Second {
        shared: i32,
        child: ChildStruct,
    },
    Third,
}

/// Returns a cloneable child struct.
pub fn make_child_struct(value: i32) -> ChildStruct {
    ChildStruct { value }
}

/// Returns the first variant of the boundary enum.
pub fn make_first_boundary_enum(unique: i32, shared: i32, child: i32) -> BoundaryEnum {
    BoundaryEnum::First {
        unique,
        shared,
        child: ChildStruct { value: child },
    }
}

/// Reads a child value through a shared reference.
pub fn child_value(child: &ChildStruct) -> i32 {
    child.value
}

/// Mutates a child value through a mutable reference.
pub fn set_child_value(child: &mut ChildStruct, value: i32) {
    child.value = value;
}

/// Mutates an owned child clone and returns it.
pub fn mutate_owned_child(mut child: ChildStruct, new_value: i32) -> ChildStruct {
    child.value = new_value;
    child
}

/// Copies a child value into the active child field of an enum.
pub fn copy_child_to_boundary_enum_child(source: &ChildStruct, target: &mut BoundaryEnum) {
    match target {
        BoundaryEnum::First { child, .. } | BoundaryEnum::Second { child, .. } => {
            child.value = source.value;
        }
        BoundaryEnum::Third => {}
    }
}
