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

/// Returns the second variant of the boundary enum.
pub fn make_second_boundary_enum(shared: i32, child: i32) -> BoundaryEnum {
    BoundaryEnum::Second {
        shared,
        child: ChildStruct { value: child },
    }
}

/// An enum whose variants share a field name with different field types.
pub enum DifferentSharedFieldEnum {
    Number { value: i32 },
    Text { value: String },
}

/// Returns the numeric variant of an enum with differently typed shared fields.
pub fn make_number_shared_field_enum(value: i32) -> DifferentSharedFieldEnum {
    DifferentSharedFieldEnum::Number { value }
}

/// Returns the string variant of an enum with differently typed shared fields.
pub fn make_text_shared_field_enum(value: String) -> DifferentSharedFieldEnum {
    DifferentSharedFieldEnum::Text { value }
}
