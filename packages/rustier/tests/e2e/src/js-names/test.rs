#[derive(Clone)]
pub struct Header {
    pub r#type: String,
    pub constructor: i32,
    pub __proto__: i32,
}

impl Header {
    pub fn r#delete(&mut self) {
        self.constructor = 0;
    }

    pub fn r#default() -> Self {
        Self {
            r#type: String::from("default"),
            constructor: 1,
            __proto__: 2,
        }
    }
}

pub fn reserved_parameters(r#type: i32, arguments: i32, r#await: i32) -> i32 {
    r#type + arguments + r#await
}
