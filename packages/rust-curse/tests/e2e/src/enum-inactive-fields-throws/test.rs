/// A point stored as the center of a visible shape.
#[derive(Clone)]
pub struct Point {
    pub x: i32,
}

/// A shape with fields that are unavailable while it is hidden.
#[derive(Clone)]
pub enum Shape {
    Circle {
        radius: i32,
        layer: i32,
        center: Point,
    },
    Square {
        side: i32,
        layer: i32,
        center: Point,
    },
    Hidden,
}

/// Returns a hidden shape with no active data fields.
pub fn make_hidden() -> Shape {
    Shape::Hidden
}
