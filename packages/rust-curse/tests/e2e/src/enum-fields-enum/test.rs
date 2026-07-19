/// A point stored as the center of a shape.
#[derive(Clone)]
pub struct Point {
    pub x: i32,
}

/// A shape that can be projected from a drawing.
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

/// Mutates the radius of a circle through a mutable shape reference.
pub fn set_circle_radius(shape: &mut Shape, new_radius: i32) {
    if let Shape::Circle { radius, .. } = shape {
        *radius = new_radius;
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

/// A drawing that optionally contains a selected shape.
pub enum Drawing {
    Selected { shape: Shape },
    Empty,
}

/// Returns a drawing with a selected circle.
pub fn make_selected_drawing(radius: i32, layer: i32, center_x: i32) -> Drawing {
    Drawing::Selected {
        shape: Shape::Circle {
            radius,
            layer,
            center: Point { x: center_x },
        },
    }
}
