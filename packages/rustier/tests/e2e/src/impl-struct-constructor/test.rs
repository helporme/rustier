#[derive(Clone)]
pub struct NoArgs;

impl NoArgs {
    pub fn new() -> Self {
        Self
    }

    pub fn works(&self) -> bool {
        true
    }
}

#[derive(Clone)]
pub struct NamedReturn {
    value: i32,
}

impl NamedReturn {
    pub fn new(value: i32) -> NamedReturn {
        NamedReturn { value }
    }

    pub fn value(&self) -> i32 {
        self.value
    }
}

#[derive(Clone)]
pub struct ReceiverNew {
    value: i32,
}

impl ReceiverNew {
    pub fn new(&self) -> Self {
        Self {
            value: self.value + 1,
        }
    }

    pub fn value(&self) -> i32 {
        self.value
    }
}

pub fn make_receiver_new(value: i32) -> ReceiverNew {
    ReceiverNew { value }
}

#[derive(Clone)]
pub struct Other {
    value: i32,
}

impl Other {
    pub fn value(&self) -> i32 {
        self.value
    }
}

pub struct WrongReturn;

impl WrongReturn {
    pub fn new(value: i32) -> Other {
        Other { value }
    }
}
