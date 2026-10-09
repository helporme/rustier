impl super::Thermometer {
    pub fn temperature(&self) -> i32 {
        self.temperature
    }

    fn private_reading(&self) -> i32 {
        self.temperature
    }

    pub(super) fn module_reading(&self) -> i32 {
        self.temperature
    }
}
