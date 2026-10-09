#[derive(Clone)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone)]
pub struct Circle {
    pub center: Point,
    pub radius: i32,
}

impl Circle {
    pub fn new(center: Point, radius: i32) -> Self {
        Self { center, radius }
    }

    pub fn centered(center: Point) -> Self {
        Self { center, radius: 1 }
    }

    pub fn replace_center(&mut self, center: Point) -> Point {
        self.center = center;
        self.center.clone()
    }

    pub fn shift_point(&self, point: &mut Point) -> i32 {
        point.x += self.radius;
        point.x
    }
}

pub fn circle_center(circle: Circle) -> Point {
    circle.center
}

pub fn shift_circle(circle: &mut Circle) -> i32 {
    circle.center.x += 1;
    circle.center.x
}

#[derive(Clone)]
pub enum Color {
    Rgb { red: i32 },
    Clear,
}

#[derive(Clone)]
pub struct Pair(pub Point, pub Color);

#[derive(Clone)]
pub enum Drawing {
    Named { point: Point, color: Color },
    Tuple(Point, Color),
}

#[derive(Clone)]
pub struct Opaque {
    value: i32,
}

impl Opaque {
    pub fn new(value: i32) -> Self {
        Self { value }
    }
}

pub fn opaque_value(value: Opaque) -> i32 {
    value.value
}

#[derive(Clone)]
pub struct Vault {
    pub value: Opaque,
}

#[derive(Clone)]
pub struct Coerce {
    pub value: i32,
}

#[derive(Clone)]
pub enum Plain {
    Unit,
}
