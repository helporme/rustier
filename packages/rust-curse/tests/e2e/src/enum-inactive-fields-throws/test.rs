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

/// Returns the fieldless third variant of the boundary enum.
pub fn make_third_boundary_enum() -> BoundaryEnum {
    BoundaryEnum::Third
}
