pub trait Read {
    fn read(&self) -> u32;
}

pub trait Marker {}

pub trait Unsupported {
    fn unsupported(&self, value: u64);
}

pub trait Unused {
    fn unused(&self) -> bool;
}
