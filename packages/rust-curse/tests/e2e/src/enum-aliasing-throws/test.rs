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

/// Returns the first variant of the boundary enum.
pub fn make_first_boundary_enum(unique: i32, shared: i32, child: i32) -> BoundaryEnum {
    BoundaryEnum::First {
        unique,
        shared,
        child: ChildStruct { value: child },
    }
}

/// Mutates two enum values through independent mutable references.
pub fn set_two_boundary_enums(first: &mut BoundaryEnum, second: &mut BoundaryEnum) {
    *first = BoundaryEnum::Third;
    *second = BoundaryEnum::Third;
}

/// Clones a shared enum value into a separately borrowed mutable enum value.
pub fn copy_boundary_enum(source: &BoundaryEnum, target: &mut BoundaryEnum) {
    *target = source.clone();
}

/// Reads an enum while mutating one of its projected child fields.
pub fn boundary_enum_and_child_value(
    parent: &BoundaryEnum,
    projected_child: &mut ChildStruct,
) -> i32 {
    projected_child.value += 1;

    match parent {
        BoundaryEnum::First { child, .. } | BoundaryEnum::Second { child, .. } => child.value,
        BoundaryEnum::Third => 0,
    }
}

/// Reads one child while mutating another.
pub fn read_and_mutate_child(read: &ChildStruct, mutate: &mut ChildStruct) -> i32 {
    mutate.value += 1;
    read.value
}
