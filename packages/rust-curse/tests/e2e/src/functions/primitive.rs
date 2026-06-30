#[allow(clippy::too_many_arguments)]
pub fn accepts_all_primitives(
    flag: bool,
    text: &str,
    i8_value: i8,
    i16_value: i16,
    i32_value: i32,
    u8_value: u8,
    u16_value: u16,
    u32_value: u32,
    f64_value: f64,
) -> bool {
    flag && text == "rust"
        && i8_value == -8
        && i16_value == -16
        && i32_value == -32
        && u8_value == 8
        && u16_value == 16
        && u32_value == 32
        && f64_value == 64.5
}

pub fn accepts_refs(flag: &bool, text: &str, i32_value: &i32, f64_value: &f64) -> bool {
    *flag && text == "curse" && *i32_value == 42 && *f64_value == 1.5
}

pub fn identity_bool(value: bool) -> bool {
    value
}

pub fn identity_i8(value: i8) -> i8 {
    value
}

pub fn identity_i16(value: i16) -> i16 {
    value
}

pub fn identity_i32(value: i32) -> i32 {
    value
}

pub fn identity_u8(value: u8) -> u8 {
    value
}

pub fn identity_u16(value: u16) -> u16 {
    value
}

pub fn identity_u32(value: u32) -> u32 {
    value
}

pub fn identity_f64(value: f64) -> f64 {
    value
}

pub fn str_eq(value: &str, expected: &str) -> bool {
    value == expected
}

pub fn return_bool() -> bool {
    true
}

pub fn return_i8() -> i8 {
    -8
}

pub fn return_i16() -> i16 {
    -16
}

pub fn return_i32() -> i32 {
    -32
}

pub fn return_u8() -> u8 {
    8
}

pub fn return_u16() -> u16 {
    16
}

pub fn return_u32() -> u32 {
    32
}

pub fn return_f64() -> f64 {
    64.5
}

pub fn no_args_no_return() {}
