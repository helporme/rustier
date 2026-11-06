//! Humble structs and enums used in e2e tests.

pub struct Message(pub &'static str);

pub struct Borrowed<'a>(pub &'a str);

#[derive(Clone, Copy)]
pub struct CopyValue(pub u32);

pub enum State {
    Ready(String),
}
