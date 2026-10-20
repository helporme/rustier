#[derive(Clone)]
pub struct Seed {
    pub value: u32,
}

#[derive(Clone)]
pub struct Wrap<T> {
    pub item: T,
}

#[derive(Clone)]
pub enum Choice {
    Value(u32),
    Empty,
}
