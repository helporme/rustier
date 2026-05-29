#[derive(Clone, Debug, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Line {
    pub start: Point,
    pub end: Point,
}
