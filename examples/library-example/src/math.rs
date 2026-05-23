#[derive(Clone)]
pub struct Circle {
    pub radius: f64,
    pub center: Point,
}

#[derive(Clone)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

pub fn is_inside(circle: &Circle, point: &Point) -> bool {
    is_inside_move(circle.clone(), point.clone())
}

pub fn is_inside_move(circle: Circle, point: Point) -> bool {
    let Circle { radius, center } = circle;
    let Point { x, y } = point;

    let dx = x - center.x;
    let dy = y - center.y;
    let distance = (dx * dx + dy * dy).sqrt();
    distance < radius
}
