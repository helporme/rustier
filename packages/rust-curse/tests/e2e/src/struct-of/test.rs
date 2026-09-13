#[derive(Clone)]
pub struct Ready;

#[derive(Clone)]
pub struct Point(pub i32, pub i32);

#[derive(Clone)]
pub struct Label {
    pub text: String,
    pub position: Point,
}

#[derive(Clone)]
pub struct Counter {
    pub value: i32,
}

impl Counter {
    pub fn read_value(&self) -> i32 {
        self.value
    }

    pub fn of(&self) -> i32 {
        self.value + 1
    }
}

#[derive(Clone)]
pub struct Ticket {
    pub number: i32,
}

impl Ticket {
    pub fn of(number: i32) -> Self {
        Self {
            number: number + 100,
        }
    }
}

pub fn point_x(point: Point) -> i32 {
    point.0
}
