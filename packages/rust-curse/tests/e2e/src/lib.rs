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
    struct_value: ValueStruct,
    struct_ref: &ValueStruct,
    struct_mut_ref: &mut ValueStruct,
) -> bool {
    string_mut_ref.push_str(" mutated");
    struct_mut_ref.value += 1;

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
}

pub fn format_mixed(text: &str, character: char, value: i32) -> String {
    format!("{text}:{character}:{value}")
}

pub fn add_mixed(left: i16, right: &i16, offset: u8) -> i32 {
    i32::from(left) + i32::from(*right) + i32::from(offset)
}

pub fn choose_mixed(flag: &bool, value: char, fallback: &char) -> char {
    if *flag { value } else { *fallback }
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

#[derive(Clone)]
pub struct ValueStruct {
    pub value: i32,
}

#[derive(Clone)]
pub struct StringFieldStruct {
    pub value: String,
}

pub struct CounterStruct {
    pub value: i32,
    counter: std::cell::Cell<u32>,
}

#[derive(Clone)]
pub struct ChildStruct {
    pub value: i32,
}

#[derive(Clone)]
pub struct ParentStruct {
    pub child: ChildStruct,
}

#[derive(Clone)]
pub struct GrandparentStruct {
    pub parent: ParentStruct,
}

pub struct NonCloneChildStruct {
    pub value: i32,
}

pub struct NonCloneParentStruct {
    pub child: NonCloneChildStruct,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct TupleStruct(i32, pub i32);

pub fn make_value_struct(value: i32) -> ValueStruct {
    ValueStruct { value }
}

pub fn make_string_field_struct(value: String) -> StringFieldStruct {
    StringFieldStruct { value }
}

pub fn string_field_value(value: &StringFieldStruct) -> String {
    value.value.clone()
}

pub fn make_counter_struct(value: i32) -> CounterStruct {
    CounterStruct {
        value,
        counter: std::cell::Cell::new(0),
    }
}

pub fn make_child_struct(value: i32) -> ChildStruct {
    ChildStruct { value }
}

pub fn make_parent_struct(value: i32) -> ParentStruct {
    ParentStruct {
        child: ChildStruct { value },
    }
}

pub fn make_grandparent_struct(value: i32) -> GrandparentStruct {
    GrandparentStruct {
        parent: ParentStruct {
            child: ChildStruct { value },
        },
    }
}

pub fn make_non_clone_parent_struct(value: i32) -> NonCloneParentStruct {
    NonCloneParentStruct {
        child: NonCloneChildStruct { value },
    }
}

pub fn make_tuple_struct(value: i32) -> TupleStruct {
    TupleStruct(0, value)
}

pub fn value_struct_value(value: &ValueStruct) -> i32 {
    value.value
}

pub fn read_counter_ref(value: &CounterStruct) -> u32 {
    let next = value.counter.get() + 1;
    value.counter.set(next);
    next
}

pub fn same_counter_refs(first: &CounterStruct, second: &CounterStruct) -> bool {
    std::ptr::eq(first, second)
}

pub fn set_counter_value(value: &mut CounterStruct, new_value: i32) {
    value.value = new_value;
}

pub fn copy_counter_value(source: &CounterStruct, target: &mut CounterStruct) {
    target.value = source.value;
}

pub fn set_two_counter_values(first: &mut CounterStruct, second: &mut CounterStruct) {
    first.value = 1;
    second.value = 2;
}

pub fn mutate_owned_value_struct(mut value: ValueStruct, new_value: i32) -> ValueStruct {
    value.value = new_value;
    value
}

pub fn describe_two_owned_value_structs(mut first: ValueStruct, mut second: ValueStruct) -> String {
    first.value += 10;
    second.value += 100;
    format!("{}:{}", first.value, second.value)
}

pub fn describe_owned_and_ref_value_struct(
    mut owned: ValueStruct,
    borrowed: &ValueStruct,
) -> String {
    owned.value += 10;
    format!("{}:{}", owned.value, borrowed.value)
}

pub fn child_value(child: &ChildStruct) -> i32 {
    child.value
}

pub fn set_child_value(child: &mut ChildStruct, value: i32) {
    child.value = value;
}

pub fn copy_child_to_parent_child(source: &ChildStruct, parent: &mut ParentStruct) {
    parent.child.value = source.value;
}

pub fn parent_child_value(parent: &ParentStruct) -> i32 {
    parent.child.value
}

pub fn parent_and_child_value(parent: &ParentStruct, child: &mut ChildStruct) -> i32 {
    child.value += 1;
    parent.child.value
}

pub fn read_and_mutate_child(read: &ChildStruct, mutate: &mut ChildStruct) -> i32 {
    mutate.value += 1;
    read.value
}

pub fn mutate_owned_child(mut child: ChildStruct, new_value: i32) -> ChildStruct {
    child.value = new_value;
    child
}

pub fn non_clone_child_value(child: &NonCloneChildStruct) -> i32 {
    child.value
}

pub fn set_non_clone_child_value(child: &mut NonCloneChildStruct, value: i32) {
    child.value = value;
}
