use serde::Serialize;

use crate::friend::Friend;

#[derive(Serialize)]
pub struct Wrap(pub String);

pub fn hello() -> (Wrap, Friend) {
    (Wrap("Meow meow!".to_owned()), Friend("mew".to_owned()))
}
