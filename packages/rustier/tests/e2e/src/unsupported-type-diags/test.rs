pub struct Text(str);

pub fn length(text: &Text) -> u32 {
    text.0.len() as u32
}

mod implementation {
    pub struct Token {
        pub value: u32,
    }
}

pub use implementation::Token;

pub fn make() -> Token {
    Token { value: 7 }
}

pub fn supported() -> u32 {
    42
}
