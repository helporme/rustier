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

pub fn accepts_string(value: String) -> bool {
    value == "owned 🦀"
}

pub fn accepts_ref_string(value: &String) -> bool {
    value == "borrowed 🦀"
}

pub fn accepts_mut_string(value: &mut String) -> bool {
    value.push_str(" mutated");
    value == "mutable 🦀 mutated"
}

#[allow(clippy::too_many_arguments)]
pub fn accepts_mixed_arguments(
    flag: bool,
    flag_ref: &bool,
    flag_mut_ref: &mut bool,
    character: char,
    character_ref: &char,
    character_mut_ref: &mut char,
    text: &str,
    text_mut_ref: &mut str,
    string: String,
    string_ref: &String,
    string_mut_ref: &mut String,
    i8_value: i8,
    i8_ref: &i8,
    i8_mut_ref: &mut i8,
    i16_value: i16,
    i16_ref: &i16,
    i16_mut_ref: &mut i16,
    i32_value: i32,
    i32_ref: &i32,
    i32_mut_ref: &mut i32,
    u8_value: u8,
    u8_ref: &u8,
    u8_mut_ref: &mut u8,
    u16_value: u16,
    u16_ref: &u16,
    u16_mut_ref: &mut u16,
    u32_value: u32,
    u32_ref: &u32,
    u32_mut_ref: &mut u32,
    f32_value: f32,
    f32_ref: &f32,
    f32_mut_ref: &mut f32,
    f64_value: f64,
    f64_ref: &f64,
    f64_mut_ref: &mut f64,
) -> bool {
    string_mut_ref.push_str(" mutated");

    flag && *flag_ref
        && *flag_mut_ref
        && character == '🦀'
        && *character_ref == '🦀'
        && *character_mut_ref == '🦀'
        && text == "str"
        && &*text_mut_ref == "mutable str"
        && string == "owned"
        && string_ref == "borrowed"
        && string_mut_ref == "mutable mutated"
        && i8_value == -8
        && *i8_ref == -8
        && *i8_mut_ref == -8
        && i16_value == -16
        && *i16_ref == -16
        && *i16_mut_ref == -16
        && i32_value == -32
        && *i32_ref == -32
        && *i32_mut_ref == -32
        && u8_value == 8
        && *u8_ref == 8
        && *u8_mut_ref == 8
        && u16_value == 16
        && *u16_ref == 16
        && *u16_mut_ref == 16
        && u32_value == 32
        && *u32_ref == 32
        && *u32_mut_ref == 32
        && f32_value == 32.5
        && *f32_ref == 32.5
        && *f32_mut_ref == 32.5
        && f64_value == 64.5
        && *f64_ref == 64.5
        && *f64_mut_ref == 64.5
}

pub fn format_mixed(text: &str, character: char, value: i32) -> String {
    format!("{text}:{character}:{value}")
}

pub fn add_mixed(left: i16, right: &i16, offset: u8) -> i32 {
    i32::from(left) + i32::from(*right) + i32::from(offset)
}

pub fn choose_mixed(flag: &bool, value: char, fallback: &char) -> char {
    if *flag {
        value
    } else {
        *fallback
    }
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

pub fn identity_char(value: char) -> char {
    value
}

pub fn identity_f32(value: f32) -> f32 {
    value
}

pub fn identity_f64(value: f64) -> f64 {
    value
}

pub fn identity_string(value: String) -> String {
    value
}

pub fn str_eq(value: &str, expected: &str) -> bool {
    value == expected
}

pub fn return_string() -> String {
    String::from("returned 🦀")
}

pub fn no_args_no_return() {}
