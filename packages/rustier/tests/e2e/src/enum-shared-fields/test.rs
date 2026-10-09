/// A point stored as the center of a shape.
#[derive(Clone)]
pub struct Point {
    pub x: i32,
}

/// A shape whose visible variants share a writable layer field.
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

/// Returns a square with the supplied side, layer, and center coordinate.
pub fn make_square(side: i32, layer: i32, center_x: i32) -> Shape {
    Shape::Square {
        side,
        layer,
        center: Point { x: center_x },
    }
}

/// A form value whose variants expose the same field name with different types.
pub enum FormValue {
    Number { value: i32 },
    Text { value: String },
}

/// Returns a numeric form value.
pub fn make_number_value(value: i32) -> FormValue {
    FormValue::Number { value }
}

/// Returns a text form value.
pub fn make_text_value(value: String) -> FormValue {
    FormValue::Text { value }
}
