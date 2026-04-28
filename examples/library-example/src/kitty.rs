pub struct Wrap(pub String);

pub fn hello() -> Wrap {
    Wrap("Meow meow!".to_owned())
}
