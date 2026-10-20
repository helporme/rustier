pub trait Pair<T, U> {
    fn pair(&self) -> bool;
}

#[derive(Clone)]
pub struct Seed(pub u32);

#[derive(Clone)]
pub struct OtherSeed(pub u32);

pub struct Widget;

impl Pair<u32, String> for Widget {
    fn pair(&self) -> bool {
        true
    }
}

impl Pair<String, u32> for Widget {
    fn pair(&self) -> bool {
        true
    }
}

impl Pair<&Seed, &mut OtherSeed> for Widget {
    fn pair(&self) -> bool {
        true
    }
}
