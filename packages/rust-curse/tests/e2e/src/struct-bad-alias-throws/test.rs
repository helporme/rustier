/// A mutable value used to exercise root-level alias detection.
pub struct CounterStruct {
    pub value: i32,
}

/// A nested value used to exercise projected alias detection.
#[derive(Clone)]
pub struct ChildStruct {
    pub value: i32,
}

/// A parent containing a projected child value.
#[derive(Clone)]
pub struct ParentStruct {
    pub child: ChildStruct,
}

/// Creates a counter with the supplied value.
pub fn make_counter_struct(value: i32) -> CounterStruct {
    CounterStruct { value }
}

/// Creates a parent whose child has the supplied value.
pub fn make_parent_struct(value: i32) -> ParentStruct {
    ParentStruct {
        child: ChildStruct { value },
    }
}

/// Copies a shared counter value into a mutably borrowed counter.
pub fn copy_counter_value(source: &CounterStruct, target: &mut CounterStruct) {
    target.value = source.value;
}

/// Mutates two independently borrowed counters.
pub fn set_two_counter_values(first: &mut CounterStruct, second: &mut CounterStruct) {
    first.value = 1;
    second.value = 2;
}

/// Reads a parent while mutating a separately projected child.
pub fn parent_and_child_value(parent: &ParentStruct, child: &mut ChildStruct) -> i32 {
    child.value += 1;
    parent.child.value
}

/// Reads one child reference while mutating another child reference.
pub fn read_and_mutate_child(read: &ChildStruct, mutate: &mut ChildStruct) -> i32 {
    mutate.value += 1;
    read.value
}
