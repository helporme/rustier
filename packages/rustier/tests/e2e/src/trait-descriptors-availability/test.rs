pub mod contracts;

#[derive(Clone)]
pub struct Widget(pub u32);

impl contracts::Read for Widget {
    fn read(&self) -> u32 {
        self.0
    }
}

impl contracts::Marker for Widget {}

impl contracts::Unsupported for Widget {
    fn unsupported(&self, _value: u64) {}
}
