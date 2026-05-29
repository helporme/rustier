#[derive(Clone, Debug, Default)]
pub struct Circle {
    pub radius: f64,
    pub center: Point,
}

#[derive(Clone, Debug, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

impl Circle {
    pub fn new(radius: f64, center: Point) -> Self {
        Self { radius, center }
    }

    pub fn contains(&self, point: &Point) -> bool {
        self.center.distance(point) <= self.radius
    }
}
