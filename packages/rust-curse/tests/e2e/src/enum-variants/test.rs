/// A two-dimensional point used by shape variants.
#[derive(Debug, Clone, PartialEq)]
pub struct Point(pub i32, pub i32);

/// A data-carrying shape enum with struct, tuple, and unit variants.
pub enum Shape {
    Circle {
        radius: f64,
        center: Point,
    },
    Oval {
        radius_x: f64,
        radius_y: f64,
        center: Point,
    },
    Point(Point),
    Line(Point, Point),
    Hidden,
}

/// Returns a circle struct variant.
pub fn make_circle(radius: f64, x: i32, y: i32) -> Shape {
    Shape::Circle {
        radius,
        center: Point(x, y),
    }
}

/// Returns an oval struct variant.
pub fn make_oval(radius_x: f64, radius_y: f64, x: i32, y: i32) -> Shape {
    Shape::Oval {
        radius_x,
        radius_y,
        center: Point(x, y),
    }
}

/// Returns a point tuple variant with one field.
pub fn make_point(x: i32, y: i32) -> Shape {
    Shape::Point(Point(x, y))
}

/// Returns a line tuple variant with two fields.
pub fn make_line(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> Shape {
    Shape::Line(Point(from_x, from_y), Point(to_x, to_y))
}

/// Returns the hidden unit variant of the data-carrying shape enum.
pub fn make_hidden() -> Shape {
    Shape::Hidden
}
