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

/// Reads the shared field through a shared enum reference.
pub fn boundary_enum_shared(value: &BoundaryEnum) -> i32 {
    match value {
        BoundaryEnum::First { shared, .. } | BoundaryEnum::Second { shared, .. } => *shared,
        BoundaryEnum::Third => 0,
    }
}

/// Reports whether two shared enum references point to the same Rust value.
pub fn same_boundary_enum_refs(first: &BoundaryEnum, second: &BoundaryEnum) -> bool {
    std::ptr::eq(first, second)
}

/// Mutates a field through a mutable enum reference.
pub fn set_boundary_enum_unique(value: &mut BoundaryEnum, new_value: i32) {
    if let BoundaryEnum::First { unique, .. } = value {
        *unique = new_value;
    }
}

/// Mutates an owned enum clone and returns it.
pub fn mutate_owned_boundary_enum(
    mut value: BoundaryEnum,
    new_unique: i32,
    new_child: i32,
) -> BoundaryEnum {
    if let BoundaryEnum::First { unique, child, .. } = &mut value {
        *unique = new_unique;
        child.value = new_child;
    }
    value
}
