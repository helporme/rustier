use rustier_ext::runtime::js::JsEnv;

pub fn accepts_env(_env: JsEnv) -> bool {
    true
}

pub fn add_with_env(left: i32, _env: JsEnv, right: &i32) -> i32 {
    left + right
}

pub fn same_env(first: JsEnv, second: JsEnv) -> bool {
    first == second
}
