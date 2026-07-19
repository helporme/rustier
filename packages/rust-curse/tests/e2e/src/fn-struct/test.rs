#[derive(Clone)]
pub struct ValueStruct {
    pub value: i32,
}

pub struct CounterStruct {
    pub value: i32,
    counter: std::cell::Cell<u32>,
}

pub fn make_value_struct(value: i32) -> ValueStruct {
    ValueStruct { value }
}

pub fn make_counter_struct(value: i32) -> CounterStruct {
    CounterStruct {
        value,
        counter: std::cell::Cell::new(0),
    }
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
