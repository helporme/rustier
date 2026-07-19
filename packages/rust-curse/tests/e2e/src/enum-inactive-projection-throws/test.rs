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

/// Replaces an enum in place with its second variant.
pub fn replace_boundary_enum_with_second(value: &mut BoundaryEnum, shared: i32, child: i32) {
    *value = BoundaryEnum::Second {
        shared,
        child: ChildStruct { value: child },
    };
}
