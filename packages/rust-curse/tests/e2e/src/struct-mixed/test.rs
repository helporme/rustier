/// A nested struct stored in the aggregate struct.
#[derive(Clone)]
pub struct NestedStruct {
    pub value: i32,
}

/// A nested enum stored in the aggregate struct.
#[derive(Clone)]
pub enum NestedEnum {
    Value { value: i32 },
    Empty,
}

/// A struct containing every supported owned field category.
#[derive(Clone)]
pub struct MixedStruct {
    pub flag: bool,
    pub character: char,
    pub text: String,
    pub i8_value: i8,
    pub i16_value: i16,
    pub i32_value: i32,
    pub u8_value: u8,
    pub u16_value: u16,
    pub u32_value: u32,
    pub f32_value: f32,
    pub f64_value: f64,
    pub struct_value: NestedStruct,
    pub enum_value: NestedEnum,
}

/// Creates a nested struct with the supplied value.
pub fn make_nested_struct(value: i32) -> NestedStruct {
    NestedStruct { value }
}

/// Creates the data-carrying nested enum variant with the supplied value.
pub fn make_nested_enum(value: i32) -> NestedEnum {
    NestedEnum::Value { value }
}

/// Creates an aggregate struct populated with representative values.
pub fn make_mixed_struct() -> MixedStruct {
    MixedStruct {
        flag: true,
        character: '🦀',
        text: "owned".to_owned(),
        i8_value: -8,
        i16_value: -16,
        i32_value: -32,
        u8_value: 8,
        u16_value: 16,
        u32_value: 32,
        f32_value: 32.5,
        f64_value: 64.5,
        struct_value: NestedStruct { value: 100 },
        enum_value: NestedEnum::Value { value: 200 },
    }
}
