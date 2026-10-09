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

/// Returns a circle with the given radius, layer, and center position.
pub fn circle(radius: i32, layer: i32, center: i32) -> Shape {
    Shape::Circle {
        radius,
        layer,
        center: Point { position: center },
    }
}

/// Reads a shape layer through a shared enum reference.
pub fn shape_layer(shape: &Shape) -> i32 {
    match shape {
        Shape::Circle { layer, .. } | Shape::Square { layer, .. } => *layer,
        Shape::Hidden => 0,
    }
}

/// Sets a circle radius through a mutable enum reference.
pub fn set_circle_radius(shape: &mut Shape, new_radius: i32) {
    if let Shape::Circle { radius, .. } = shape {
        *radius = new_radius;
    }
}

/// Replaces a shape in place with a square.
pub fn replace_with_square(shape: &mut Shape, layer: i32, center: i32) {
    *shape = Shape::Square {
        layer,
        center: Point { position: center },
    };
}

/// A canvas containing an enum-valued shape field.
#[derive(Clone)]
pub struct Canvas {
    pub shape: Shape,
}

/// Returns a canvas containing a circle.
pub fn canvas_with_circle(radius: i32, layer: i32, center: i32) -> Canvas {
    Canvas {
        shape: Shape::Circle {
            radius,
            layer,
            center: Point { position: center },
        },
    }
}
