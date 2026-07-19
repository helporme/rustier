/// A point stored as the center of a shape.
#[derive(Clone)]
pub struct Point {
    pub x: i32,
}

/// The enum type expected by functions and fields in this fixture.
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

/// Reads the layer shared by visible shape variants.
pub fn read_shape_layer(shape: &Shape) -> i32 {
    match shape {
        Shape::Circle { layer, .. } | Shape::Square { layer, .. } => *layer,
        Shape::Hidden => 0,
    }
}

/// A canvas containing a shape-valued struct field.
#[derive(Clone)]
pub struct Canvas {
    pub shape: Shape,
}

/// Returns a canvas containing a circle.
pub fn make_canvas(radius: i32, layer: i32, center_x: i32) -> Canvas {
    Canvas {
        shape: Shape::Circle {
            radius,
            layer,
            center: Point { x: center_x },
        },
    }
}

/// A drawing containing a shape-valued enum field.
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

/// A non-C-like enum used as the actual wrong enum type.
pub enum Color {
    Rgb { red: u8, green: u8, blue: u8 },
}

/// Returns an RGB color with named component fields.
pub fn make_rgb_color(red: u8, green: u8, blue: u8) -> Color {
    Color::Rgb { red, green, blue }
}
