#[derive(Clone)]
pub struct Point {
    // primitive field
    pub x: f64,
    pub y: f64,
}

#[derive(Clone)]
pub struct Line {
    pub start: Point,
    pub end: Point,
}

#[derive(Clone)]
pub struct Triangle {
    // struct field
    pub line: Line,
    pub point: Point,
}

// inherent impl
impl Point {
    // ctor
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    // reads self
    pub fn length(&self) -> f64 {
        (self.x.powi(2) + self.y.powi(2)).sqrt()
    }

    // modifies self
    pub fn scale(&mut self, factor: f64) {
        self.x *= factor;
        self.y *= factor;
    }
}

// 2nd inherent impl
impl Point {
    // invisible
    #[allow(dead_code)]
    pub(crate) fn translate(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
    }
}

impl Line {
    // accept coercable args, js keyword as method
    pub fn from(start: Point, end: Point) -> Self {
        Self { start, end }
    }

    // collide with generated of method
    pub fn of(point: Point) -> Self {
        Self {
            start: point.clone(),
            end: point,
        }
    }
}
