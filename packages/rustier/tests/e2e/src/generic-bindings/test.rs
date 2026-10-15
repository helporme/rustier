#[derive(Clone)]
pub struct Parcel<T> {
    pub item: T,
}

impl<T> Parcel<T> {
    pub fn new(item: T) -> Self {
        Self { item }
    }

    pub fn take(&mut self) -> T
    where
        T: Default,
    {
        std::mem::take(&mut self.item)
    }
}

impl<T: Clone> Parcel<T> {
    pub fn duplicate(&self) -> Self {
        self.clone()
    }
}

impl Parcel<i32> {
    pub fn doubled(&self) -> i32 {
        self.item * 2
    }
}

pub struct Ticket {
    pub number: i32,
}

#[derive(Clone)]
pub enum Delivery<T, E> {
    Arrived { item: T },
    Failed(E),
}

pub fn number_parcel(item: i32) -> Parcel<i32> {
    Parcel { item }
}

pub fn text_parcel(item: String) -> Parcel<String> {
    Parcel { item }
}

pub fn nested_parcel(item: i32) -> Parcel<Parcel<i32>> {
    Parcel {
        item: Parcel { item },
    }
}

pub fn ticket_parcel(number: i32) -> Parcel<Ticket> {
    Parcel {
        item: Ticket { number },
    }
}

pub fn delivered_number(item: i32) -> Delivery<i32, String> {
    Delivery::Arrived { item }
}

pub fn delivered_text(item: String) -> Delivery<String, i32> {
    Delivery::Arrived { item }
}

pub fn read_number(parcel: &Parcel<i32>) -> i32 {
    parcel.item
}
