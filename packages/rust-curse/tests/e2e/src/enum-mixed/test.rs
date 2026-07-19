/// A nested struct stored in data-carrying enum variants.
#[derive(Clone)]
pub struct NestedStruct {
    pub value: i32,
}

/// A nested enum stored in data-carrying enum variants.
#[derive(Clone)]
pub enum NestedEnum {
    Value { value: i32 },
    Empty,
}

/// An enum combining unit, tuple, and struct variants with every supported owned field category.
pub enum MixedEnum {
    Unit,
    Tuple(
        bool,
        char,
        String,
        i8,
        i16,
        i32,
        u8,
        u16,
        u32,
        f32,
        f64,
        NestedStruct,
        NestedEnum,
    ),
    Struct {
        flag: bool,
        character: char,
        text: String,
        i8_value: i8,
        i16_value: i16,
        i32_value: i32,
        u8_value: u8,
        u16_value: u16,
        u32_value: u32,
        f32_value: f32,
        f64_value: f64,
        struct_value: NestedStruct,
        enum_value: NestedEnum,
    },
}

/// Creates the unit variant of the aggregate enum.
pub fn make_unit_variant() -> MixedEnum {
    MixedEnum::Unit
}

/// Creates the tuple variant with representative values for every supported field category.
pub fn make_tuple_variant() -> MixedEnum {
    MixedEnum::Tuple(
        true,
        '🦀',
        "tuple".to_owned(),
        -8,
        -16,
        -32,
        8,
        16,
        32,
        32.5,
        64.5,
        NestedStruct { value: 100 },
        NestedEnum::Value { value: 200 },
    )
}

/// Creates the struct variant with representative values for every supported field category.
pub fn make_struct_variant() -> MixedEnum {
    MixedEnum::Struct {
        flag: false,
        character: '🚀',
        text: "struct".to_owned(),
        i8_value: -18,
        i16_value: -116,
        i32_value: -132,
        u8_value: 18,
        u16_value: 116,
        u32_value: 132,
        f32_value: 42.5,
        f64_value: 84.5,
        struct_value: NestedStruct { value: 300 },
        enum_value: NestedEnum::Value { value: 400 },
    }
}
