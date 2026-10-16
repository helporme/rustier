#[derive(Clone)]
pub struct Counter {
    pub value: i32,
}

impl Counter {
    pub fn add(&mut self, snapshot: Self) {
        self.value += snapshot.value;
    }
}

#[derive(Clone)]
pub struct Ledger {
    pub counter: Counter,
}

pub fn add_owned_first(snapshot: Counter, target: &mut Counter) {
    target.value += snapshot.value;
}

pub fn add_owned_last(target: &mut Counter, snapshot: Counter) {
    target.value += snapshot.value;
}

pub fn add_and_return(target: &mut Counter, snapshot: Counter) -> Counter {
    target.value += snapshot.value;

    target.clone()
}

pub fn checked_add(target: &mut Counter, snapshot: Counter, accepted: bool) -> Result<i32, String> {
    if !accepted {
        return Err(String::from("rejected"));
    }

    target.value += snapshot.value;

    Ok(target.value)
}

pub fn add_projected(target: &mut Ledger, snapshot: Counter) -> i32 {
    target.counter.value += snapshot.value;

    target.counter.value
}

pub fn read_twice(first: &Counter, second: &Counter) -> i32 {
    first.value + second.value
}

pub fn set_after_check(target: &mut Counter, value: i8) {
    target.value = value as i32;
}
