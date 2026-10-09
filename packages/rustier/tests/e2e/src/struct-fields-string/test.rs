/// A structure containing an owned string field.
#[derive(Clone)]
pub struct StringFieldStruct {
    pub value: String,
}

/// Creates a string-field structure with the supplied value.
pub fn make_string_field_struct(value: String) -> StringFieldStruct {
    StringFieldStruct { value }
}

/// Returns a copy of the structure's string field.
pub fn string_field_value(value: &StringFieldStruct) -> String {
    value.value.clone()
}
