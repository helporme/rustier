/// A point projected from a shape variant.
#[derive(Clone)]
pub struct Point {
    pub x: i32,
}

/// A shape used to verify aliasing checks across enum wrappers and projected fields.
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

/// Mutates two shapes through independent mutable references.
pub fn hide_two_shapes(first: &mut Shape, second: &mut Shape) {
    *first = Shape::Hidden;
    *second = Shape::Hidden;
}

/// Clones a shared shape into a separately borrowed mutable shape.
pub fn copy_shape(source: &Shape, target: &mut Shape) {
    *target = source.clone();
}

/// Reads a shape while mutating one of its projected centers.
pub fn read_shape_and_mutate_center(parent: &Shape, projected_center: &mut Point) -> i32 {
    projected_center.x += 1;

    match parent {
        Shape::Circle { center, .. } | Shape::Square { center, .. } => center.x,
        Shape::Hidden => 0,
    }
}

/// Reads one center while mutating another.
pub fn read_and_mutate_center(read: &Point, mutate: &mut Point) -> i32 {
    mutate.x += 1;
    read.x
}
