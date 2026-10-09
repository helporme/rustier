#[derive(Clone)]
pub struct Widget {
    id: i32,
}

impl Widget {
    pub const DEFAULT_ID: i32 = 0;

    pub fn new(id: i32) -> Self {
        Self { id }
    }

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn generic<T>(&self, value: T) -> T {
        value
    }

    pub unsafe fn dangerous(&self) -> i32 {
        self.id
    }

    pub async fn asynchronous(&self) -> i32 {
        self.id
    }
}

pub trait Identity {
    type Output;

    const KIND: &'static str;

    fn identity(&self) -> Self::Output;
}

impl Identity for Widget {
    type Output = i32;

    const KIND: &'static str = "widget";

    fn identity(&self) -> Self::Output {
        self.id
    }
}

pub trait Blanket {
    fn blanket(&self) -> bool;
}

impl<T> Blanket for T {
    fn blanket(&self) -> bool {
        true
    }
}

pub unsafe trait UnsafeMarker {
    fn marked(&self) -> bool;
}

unsafe impl UnsafeMarker for Widget {
    fn marked(&self) -> bool {
        true
    }
}

#[derive(Clone)]
pub struct Generic<T>(pub T);

impl<T> Generic<T> {
    pub fn value(&self) -> &T {
        &self.0
    }
}

impl Generic<i32> {
    pub fn concrete_value(&self) -> i32 {
        self.0
    }
}
