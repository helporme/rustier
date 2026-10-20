pub trait Read {
    fn read(&self) -> u32;
}

impl Read for super::Widget {
    fn read(&self) -> u32 {
        self.0 + 1
    }
}
