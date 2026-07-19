/// A position projected through a shape variant.
#[derive(Clone)]
pub struct Point {
    pub position: i32,
}

/// A geometric shape with variant-specific fields.
#[derive(Clone)]
pub enum Shape {
    Circle {
        radius: i32,
        layer: i32,
        center: Point,
    },
    Square {
        layer: i32,
        center: Point,
    },
    Hidden,
}

/// Returns a point at the given position.
pub fn point(position: i32) -> Point {
    Point { position }
}

/// Returns a circle with the given radius, layer, and center position.
pub fn circle(radius: i32, layer: i32, center: i32) -> Shape {
    Shape::Circle {
        radius,
        layer,
        center: Point { position: center },
    }
}

/// Reads a point position through a shared reference.
pub fn point_position(point: &Point) -> i32 {
    point.position
}

/// Sets a point position through a mutable reference.
pub fn set_point_position(point: &mut Point, position: i32) {
    point.position = position;
}

/// Moves an owned point clone to a new position and returns it.
pub fn move_owned_point(mut point: Point, new_position: i32) -> Point {
    point.position = new_position;
    point
}

/// Copies a point position into the center of the active shape variant.
pub fn copy_point_to_shape_center(source: &Point, target: &mut Shape) {
    match target {
        Shape::Circle { center, .. } | Shape::Square { center, .. } => {
            center.position = source.position;
        }
        Shape::Hidden => {}
    }
}
