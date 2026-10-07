//@ api: wasm, node
//@ diagnostic-code: rust_curse::non_clonable_moved_param

pub fn option() -> Option<String> {
    None
}

pub fn consume(value: Option<String>) {
    drop(value);
}
