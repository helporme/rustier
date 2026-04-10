use serde::Serialize;

#[derive(Serialize)]
pub struct Wrap(pub String);

pub fn hello() -> Wrap {
    Wrap("Meow meow!".to_owned())
}
