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

/// Reads the shared field through a shared enum reference.
pub fn boundary_enum_shared(value: &BoundaryEnum) -> i32 {
    match value {
        BoundaryEnum::First { shared, .. } | BoundaryEnum::Second { shared, .. } => *shared,
        BoundaryEnum::Third => 0,
    }
}

/// A struct containing an enum-valued field.
#[derive(Clone)]
pub struct BoundaryEnumHolder {
    pub value: BoundaryEnum,
}

/// Returns a struct containing the first boundary enum variant.
pub fn make_boundary_enum_holder(unique: i32, shared: i32, child: i32) -> BoundaryEnumHolder {
    BoundaryEnumHolder {
        value: BoundaryEnum::First {
            unique,
            shared,
            child: ChildStruct { value: child },
        },
    }
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

pub enum Shape {
    Unit,
}

/// Returns the unit variant of another enum type.
pub fn make_unit_shape() -> Shape {
    Shape::Unit
}
