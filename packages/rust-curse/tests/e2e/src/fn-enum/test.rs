/// Represents a point on the horizontal axis used as a shape center.
#[derive(Clone)]
pub struct Point {
    pub x: i32,
}

/// Represents a geometric shape passed across the JavaScript bridge.
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

/// Constructs a circle with the provided radius, layer, and horizontal center position.
pub fn circle(radius: i32, layer: i32, center_x: i32) -> Shape {
    Shape::Circle {
        radius,
        layer,
        center: Point { x: center_x },
    }
}

/// Reads the rendering layer through a shared shape reference.
pub fn shape_layer(shape: &Shape) -> i32 {
    match shape {
        Shape::Circle { layer, .. } | Shape::Square { layer, .. } => *layer,
        Shape::Hidden => 0,
    }
}

/// Reports whether two shared shape references point to the same Rust value.
pub fn same_shape_refs(first: &Shape, second: &Shape) -> bool {
    std::ptr::eq(first, second)
}

/// Sets a circle's radius through a mutable shape reference.
pub fn set_circle_radius(shape: &mut Shape, radius: i32) {
    if let Shape::Circle {
        radius: current_radius,
        ..
    } = shape
    {
        *current_radius = radius;
    }
}

/// Resizes and repositions an owned circle clone before returning it.
pub fn resize_owned_circle(mut shape: Shape, radius: i32, center_x: i32) -> Shape {
    if let Shape::Circle {
        radius: current_radius,
        center,
        ..
    } = &mut shape
    {
        *current_radius = radius;
        center.x = center_x;
    }
    shape
}
