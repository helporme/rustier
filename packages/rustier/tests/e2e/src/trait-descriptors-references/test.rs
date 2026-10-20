#[derive(Clone)]
pub struct Seed {
    pub value: u32,
}

#[derive(Clone)]
pub struct OtherSeed {
    pub value: u32,
}

#[derive(Clone)]
pub struct Widget(pub u32);

impl From<&Seed> for Widget {
    fn from(value: &Seed) -> Self {
        Self(value.value)
    }
}

impl From<&mut Seed> for Widget {
    fn from(value: &mut Seed) -> Self {
        value.value += 1;
        Self(value.value)
    }
}

impl From<&mut OtherSeed> for Widget {
    fn from(value: &mut OtherSeed) -> Self {
        value.value += 1;
        Self(value.value)
    }
}
