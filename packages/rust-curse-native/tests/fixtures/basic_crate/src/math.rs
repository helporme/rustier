/// A 2D point.
pub struct Point {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
}

/// Adds two points component-wise.
pub fn add(a: &Point, b: &Point) -> Point {
    Point {
        x: a.x + b.x,
        y: a.y + b.y,
    }
}
