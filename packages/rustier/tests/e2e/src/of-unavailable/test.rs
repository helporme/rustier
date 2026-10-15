pub struct NotClone {
    pub value: i32,
}

#[derive(Clone)]
pub struct PrivateField {
    pub value: i32,
    hidden: i32,
}

#[derive(Clone)]
pub struct UnsupportedField {
    pub value: i32,
    pub values: u64,
}

#[non_exhaustive]
#[derive(Clone)]
pub struct OpenRecord {
    pub value: i32,
}

// Clone on the owner alone is insufficient: its child must be passable by Clone.
pub struct Owner {
    pub child: NotClone,
}

impl Clone for Owner {
    fn clone(&self) -> Self {
        Self {
            child: NotClone {
                value: self.child.value,
            },
        }
    }
}

pub enum NotCloneEnum {
    Empty,
    Value { value: i32 },
}

#[derive(Clone)]
pub enum ClosedEnum {
    #[non_exhaustive]
    Empty,
    #[non_exhaustive]
    Tuple(i32),
    #[non_exhaustive]
    Value { value: i32 },
}

pub fn closed_tuple(value: i32) -> ClosedEnum {
    ClosedEnum::Tuple(value)
}

#[derive(Clone)]
pub struct Shadowed {
    pub value: i32,
}

impl Shadowed {
    pub fn value(&self) -> i32 {
        self.value
    }
}
