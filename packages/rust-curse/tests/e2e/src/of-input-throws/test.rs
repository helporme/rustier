#[derive(Clone)]
pub struct Point(pub i32, pub i32);

#[derive(Clone)]
pub struct OtherPoint(pub i32, pub i32);

#[derive(Clone)]
pub struct Mixed {
    pub position: Point,
    pub count: i8,
    pub enabled: bool,
    pub text: String,
}

#[derive(Clone)]
pub struct Label {
    pub position: Point,
}

#[derive(Clone)]
pub enum Shape {
    Unit,
    Circle {
        center: Point,
    },
    Hidden {
        label: i32,
    },
    #[non_exhaustive]
    Reserved {
        value: i32,
    },
}

impl Shape {
    pub fn label(&self) -> i32 {
        0
    }
}

#[derive(Clone)]
pub enum Message {
    Text { r#type: String },
    Empty,
}

pub fn point_x(point: Point) -> i32 {
    point.0
}
