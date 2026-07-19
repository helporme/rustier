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

/// Mutates a field through a mutable enum reference.
pub fn set_boundary_enum_unique(value: &mut BoundaryEnum, new_value: i32) {
    if let BoundaryEnum::First { unique, .. } = value {
        *unique = new_value;
    }
}

/// Replaces an enum in place with its second variant.
pub fn replace_boundary_enum_with_second(value: &mut BoundaryEnum, shared: i32, child: i32) {
    *value = BoundaryEnum::Second {
        shared,
        child: ChildStruct { value: child },
    };
}

/// An enum containing another enum as a variant field.
pub enum BoundaryEnumContainer {
    Occupied { value: BoundaryEnum },
    Empty,
}

/// Returns an enum variant containing the first boundary enum variant.
pub fn make_boundary_enum_container(unique: i32, shared: i32, child: i32) -> BoundaryEnumContainer {
    BoundaryEnumContainer::Occupied {
        value: BoundaryEnum::First {
            unique,
            shared,
            child: ChildStruct { value: child },
        },
    }
}
