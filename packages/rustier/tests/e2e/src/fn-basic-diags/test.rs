pub fn supported() -> i32 {
    42
}

pub fn unsupported_primitive_param(_: u64) {}

pub fn generic_fn<T>(value: T) -> T {
    value
}

pub fn borrowed_return(value: &i32) -> &i32 {
    value
}

pub async fn async_fn() {}
