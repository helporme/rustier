/// A nested struct used to exercise function arguments.
#[derive(Clone)]
pub struct ValueStruct {
    pub value: i32,
}

/// A nested enum used to exercise function arguments.
#[derive(Clone)]
pub enum ValueEnum {
    Value { value: i32 },
    Empty,
}

/// Creates a nested struct with the supplied value.
pub fn make_value_struct(value: i32) -> ValueStruct {
    ValueStruct { value }
}

/// Creates the data-carrying nested enum variant with the supplied value.
pub fn make_value_enum(value: i32) -> ValueEnum {
    ValueEnum::Value { value }
}

/// Accepts every supported argument category by value and reference.
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
    struct_value: ValueStruct,
    struct_ref: &ValueStruct,
    struct_mut_ref: &mut ValueStruct,
    enum_value: ValueEnum,
    enum_ref: &ValueEnum,
    enum_mut_ref: &mut ValueEnum,
) -> bool {
    string_mut_ref.push_str(" mutated");
    struct_mut_ref.value += 1;
    if let ValueEnum::Value { value } = enum_mut_ref {
        *value += 1;
    }

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
        && struct_value.value == 1
        && struct_ref.value == 2
        && struct_mut_ref.value == 4
        && matches!(enum_value, ValueEnum::Value { value: 1 })
        && matches!(enum_ref, ValueEnum::Value { value: 2 })
        && matches!(enum_mut_ref, ValueEnum::Value { value: 4 })
}

/// Formats mixed string, character, and numeric arguments.
pub fn format_mixed(text: &str, character: char, value: i32) -> String {
    format!("{text}:{character}:{value}")
}

/// Adds mixed signed, referenced, and unsigned numeric arguments.
pub fn add_mixed(left: i16, right: &i16, offset: u8) -> i32 {
    i32::from(left) + i32::from(*right) + i32::from(offset)
}

/// Chooses between character arguments using a referenced boolean.
pub fn choose_mixed(flag: &bool, value: char, fallback: &char) -> char {
    if *flag { value } else { *fallback }
}
