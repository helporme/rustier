pub struct MsgRef {
    name: String,
}

impl MsgRef {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

pub fn hello_ref(msg: &MsgRef) -> String {
    format!("Hello, {}!", msg.name)
}

pub fn hello_copy(msg: MsgRef) -> String {
    format!("Hello, {}!", msg.name)
}
