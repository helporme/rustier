/// A point projected from a shape variant.
#[derive(Clone)]
pub struct Point {
    pub x: i32,
}

/// A shape whose projected fields are valid only while their variant remains active.
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

/// Returns a circle with the supplied radius, layer, and center coordinate.
pub fn make_circle(radius: i32, layer: i32, center_x: i32) -> Shape {
    Shape::Circle {
        radius,
        layer,
        center: Point { x: center_x },
    }
}

/// Replaces a shape in place with a square.
pub fn replace_with_square(shape: &mut Shape, side: i32, layer: i32, center_x: i32) {
    *shape = Shape::Square {
        side,
        layer,
        center: Point { x: center_x },
    };
}
