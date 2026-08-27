mod adjustments;
mod readings;

#[derive(Clone)]
pub struct Thermometer {
    temperature: i32,
}

impl Thermometer {
    pub fn new(temperature: i32) -> Self {
        Self { temperature }
    }
}
