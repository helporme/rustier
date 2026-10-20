pub fn current() -> u32 {
    42
}

#[deprecated(note = "use current")]
pub fn old_function() -> u32 {
    1
}

#[derive(Clone)]
pub struct Example {
    pub value: u32,
}

impl Example {
    #[deprecated(note = "use create")]
    pub fn new() -> Self {
        Self { value: 0 }
    }

    pub fn create(value: u32) -> Self {
        Self { value }
    }

    pub fn current(&self) -> u32 {
        self.value
    }

    #[doc(hidden)]
    pub fn hidden(&self) -> u32 {
        self.value + 1
    }

    #[deprecated(note = "use current")]
    pub fn old(&self) -> u32 {
        self.value
    }

    #[deprecated(note = "use current")]
    pub fn old_static() -> u32 {
        1
    }
}

pub trait Legacy {
    #[deprecated(note = "use current_trait")]
    fn old_trait(&self) -> u32 {
        1
    }

    #[doc(hidden)]
    fn current_trait(&self) -> u32 {
        42
    }
}

impl Legacy for Example {}
