#[derive(Clone)]
pub struct Point(pub i32, pub i32);

#[derive(Clone)]
pub enum Shape {
    Unit,
    Circle { center: Point, radius: f64 },
    Rect(Point, Point),
}

#[derive(Clone)]
pub enum Message {
    Text { r#type: String, body: String },
    Empty,
}

#[derive(Clone)]
pub enum Request {
    HTTPRequest { id: i32 },
    HttpRequest { id: i32 },
}

#[derive(Clone)]
pub enum Status {
    Ready,
    Failed { code: i32 },
}

impl Status {
    pub fn of(code: i32) -> Self {
        Self::Failed { code }
    }
}

// A non-exhaustive parent does not prevent constructing exhaustive variants.
#[non_exhaustive]
#[derive(Clone)]
pub enum Progress {
    Started,
    Percent {
        value: i32,
    },
    Hidden {
        label: i32,
    },
    #[non_exhaustive]
    Reserved {
        value: i32,
    },
}

impl Progress {
    pub fn label(&self) -> i32 {
        match self {
            Self::Hidden { label } => *label,
            _ => 0,
        }
    }
}
