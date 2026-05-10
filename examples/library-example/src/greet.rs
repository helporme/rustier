pub struct MsgRef {
    name: String,
}

impl MsgRef {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

pub struct MsgCopy {
    pub name: String,
}

pub fn hi_ref(msg: &MsgRef) -> String {
    format!("Hello form ref, {}!", msg.name)
}

pub fn hi_copy(msg: MsgCopy) -> String {
    format!("Hello from copy, {}!", msg.name)
}

pub fn hi_copy_err(msg: MsgRef) -> String {
    format!("Hello, {}!", msg.name)
}
