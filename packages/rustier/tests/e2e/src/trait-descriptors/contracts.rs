pub trait Read {
    fn read(&self) -> u32;
}

pub trait Convert<T> {
    fn convert(value: T) -> Self;
}

pub trait Probe<T: ?Sized> {
    fn probe(&self, value: &T) -> bool;
}
