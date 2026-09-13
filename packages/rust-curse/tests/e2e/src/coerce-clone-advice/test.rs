pub struct NeedsCloneStruct {
    pub value: i32,
}

pub struct NeedsCloneTuple(pub i32);

pub struct NeedsCloneUnit;

pub enum NeedsCloneEnum {
    Unit,
    Value { value: i32 },
}

#[non_exhaustive]
pub enum NeedsCloneOpenEnum {
    Unit,
    #[non_exhaustive]
    Reserved {
        value: i32,
    },
}

pub struct NeedsCloneShadowed {
    pub value: i32,
}

impl NeedsCloneShadowed {
    pub fn value(&self) -> i32 {
        self.value
    }
}

#[derive(Clone)]
pub struct DerivedClone {
    pub value: i32,
}

pub struct ManualClone;

impl Clone for ManualClone {
    fn clone(&self) -> Self {
        Self
    }
}

pub struct PrivateFields {
    value: i32,
}

pub struct UnsupportedFields {
    pub values: Vec<i32>,
}

#[non_exhaustive]
pub struct NonExhaustiveStruct {
    pub value: i32,
}

pub struct NonCloneField {
    pub child: NeedsCloneStruct,
}

pub enum NoCoercibleVariants {
    #[non_exhaustive]
    Value { value: i32 },
}
