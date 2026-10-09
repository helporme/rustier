impl super::Thermometer {
    pub fn adjust(&mut self, amount: i32) {
        self.temperature += amount;
    }

    pub(crate) fn reset(&mut self) {
        self.temperature = 0;
    }
}
