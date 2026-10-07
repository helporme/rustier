//@ api: wasm, node
//@ diagnostic-code: rust_curse::non_clonable_moved_param

pub fn nested(value: Option<Option<String>>) -> Option<Option<String>> {
    value
}

pub fn result(value: Result<Option<String>, String>) -> Result<Option<String>, String> {
    value
}

pub fn vector(value: Vec<String>) -> Vec<String> {
    value
}
