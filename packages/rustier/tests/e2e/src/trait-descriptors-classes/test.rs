pub mod keys;
pub mod other_keys;

#[derive(Clone)]
pub struct Widget(pub u32);

impl From<keys::Seed> for Widget {
    fn from(value: keys::Seed) -> Self {
        Self(value.value)
    }
}

impl From<other_keys::Seed> for Widget {
    fn from(value: other_keys::Seed) -> Self {
        Self(value.value + 1)
    }
}

impl From<keys::Wrap<u32>> for Widget {
    fn from(value: keys::Wrap<u32>) -> Self {
        Self(value.item)
    }
}

impl From<keys::Wrap<keys::Seed>> for Widget {
    fn from(value: keys::Wrap<keys::Seed>) -> Self {
        Self(value.item.value)
    }
}

impl From<keys::Wrap<keys::Wrap<u32>>> for Widget {
    fn from(value: keys::Wrap<keys::Wrap<u32>>) -> Self {
        Self(value.item.item)
    }
}

impl From<keys::Choice> for Widget {
    fn from(value: keys::Choice) -> Self {
        match value {
            keys::Choice::Value(value) => Self(value),
            keys::Choice::Empty => Self(0),
        }
    }
}
