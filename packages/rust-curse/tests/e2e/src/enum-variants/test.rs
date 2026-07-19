#[derive(Debug, Clone, PartialEq)]
pub struct Point(pub i32, pub i32);

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
    Unit,
}

/// Returns a struct variant with primitive and struct fields.
pub fn make_circle_shape(radius: f64, x: i32, y: i32) -> Shape {
    Shape::Circle {
        radius,
        center: Point(x, y),
    }
}

/// Returns a second struct variant that shares a field with the circle variant.
pub fn make_oval_shape(radius_x: f64, radius_y: f64, x: i32, y: i32) -> Shape {
    Shape::Oval {
        radius_x,
        radius_y,
        center: Point(x, y),
    }
}

/// Returns a tuple variant containing one struct field.
pub fn make_point_shape(x: i32, y: i32) -> Shape {
    Shape::Point(Point(x, y))
}

/// Returns a tuple variant containing two fields of the same type.
pub fn make_line_shape(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> Shape {
    Shape::Line(Point(from_x, from_y), Point(to_x, to_y))
}

/// Returns the unit variant of a data-carrying enum.
pub fn make_unit_shape() -> Shape {
    Shape::Unit
}
