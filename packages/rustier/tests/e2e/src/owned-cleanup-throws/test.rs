use std::cell::Cell;

thread_local! {
    static DROPS: Cell<u32> = const { Cell::new(0) };
}

pub struct Resource {
    value: i32,
}

impl Resource {
    pub fn new(value: i32) -> Self {
        Self { value }
    }

    pub fn value(&self) -> i32 {
        self.value
    }
}

impl Clone for Resource {
    fn clone(&self) -> Self {
        Self { value: self.value }
    }
}

impl Drop for Resource {
    fn drop(&mut self) {
        DROPS.with(|drops| drops.set(drops.get() + 1));
    }
}

#[derive(Clone)]
pub struct Shipment {
    pub resource: Resource,
    pub count: i8,
}

pub fn drops() -> u32 {
    DROPS.with(Cell::get)
}

pub fn consume(resource: Resource, count: i8) -> i32 {
    resource.value + count as i32
}
