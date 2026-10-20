pub mod contracts;
pub mod other_contracts;

#[derive(Clone)]
pub struct Widget(pub u32);

impl From<u32> for Widget {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<f64> for Widget {
    fn from(value: f64) -> Self {
        Self(value as u32)
    }
}

impl From<String> for Widget {
    fn from(value: String) -> Self {
        Self(value.len() as u32)
    }
}

impl From<&str> for Widget {
    fn from(value: &str) -> Self {
        Self(value.len() as u32)
    }
}

impl contracts::Read for Widget {
    fn read(&self) -> u32 {
        self.0
    }
}

impl contracts::Convert<u32> for Widget {
    fn convert(value: u32) -> Self {
        Self(value)
    }
}

impl contracts::Convert<String> for Widget {
    fn convert(value: String) -> Self {
        Self(value.len() as u32)
    }
}

impl contracts::Probe<str> for Widget {
    fn probe(&self, value: &str) -> bool {
        !value.is_empty()
    }
}
