#[derive(Clone)]
pub struct Point {
    // primitive field
    pub x: f64,
    pub y: f64,
}

#[derive(Clone)]
pub struct Line {
    pub start: Point,
    pub end: Point,
}

#[derive(Clone)]
pub struct Triangle {
    // struct field
    pub line: Line,
    pub point: Point,
}

// inherent impl
impl Point {
    // ctor
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    // reads self
    pub fn length(&self) -> f64 {
        (self.x.powi(2) + self.y.powi(2)).sqrt()
    }

    // modifies self
    pub fn scale(&mut self, factor: f64) {
        self.x *= factor;
        self.y *= factor;
    }

    // mutates self through a by-ref struct param
    pub fn add_assign(&mut self, other: &Point) {
        self.x += other.x;
        self.y += other.y;
    }
}

// 2nd inherent impl
impl Point {
    // invisible
    #[allow(dead_code)]
    pub(crate) fn translate(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
    }
}

impl Line {
    // accept coercable args, js keyword as method
    pub fn from(start: Point, end: Point) -> Self {
        Self { start, end }
    }

    // collide with generated of method
    pub fn of(point: Point) -> Self {
        Self {
            start: point.clone(),
            end: point,
        }
    }
}

// every napi-supported primitive field, including string
#[derive(Clone)]
pub struct Primitives {
    pub flag: bool,
    pub text: String,
    pub sbyte: i8,
    pub byte: u8,
    pub short: i16,
    pub ushort: u16,
    pub int: i32,
    pub uint: u32,
    pub big: f64,
}

// `new` with `&self` -> not a ctor, stays an instance method
#[derive(Clone)]
pub struct SelfNew {
    pub v: f64,
}

impl SelfNew {
    pub fn new(&self) -> Self {
        Self { v: self.v }
    }
}

// `new` with `&mut self` -> not a ctor, stays an instance method
#[derive(Clone)]
pub struct MutSelfNew {
    pub v: f64,
}

impl MutSelfNew {
    pub fn new(&mut self) -> Self {
        self.v += 1.0;
        Self { v: self.v }
    }
}

// `new` that does not return Self -> not a ctor, stays a static method
#[derive(Clone)]
pub struct StaticNew {
    pub v: f64,
}

impl StaticNew {
    pub fn new(x: f64) -> f64 {
        x * 2.0
    }
}

// not clonable -> usable as a field type, but the parent's setter is suppressed
pub struct NoClone {
    pub v: f64,
}

impl NoClone {
    pub fn new(v: f64) -> Self {
        Self { v }
    }
}

// holds a non-clonable struct field -> getter only, no setter
pub struct Wrapper {
    pub inner: NoClone,
}

impl Wrapper {
    pub fn new(v: f64) -> Self {
        Self {
            inner: NoClone { v },
        }
    }
}

// incompatible field type -> the whole struct is skipped
pub struct BadField {
    pub items: Vec<f64>,
}

// unit struct -> skipped
pub struct Marker;

// tuple struct -> skipped
pub struct Pair(pub f64, pub f64);

// pub field of a skipped (unit) struct -> skipped
pub struct Holder {
    pub mark: Marker,
}

// accepts a skipped (tuple) struct -> skipped
pub fn use_pair(p: Pair) -> f64 {
    p.0
}

// not pub -> not exported
#[allow(dead_code)]
fn private_fn() -> f64 {
    42.0
}

// primitives by value
pub fn add(a: f64, b: f64) -> f64 {
    a + b
}

// `&str` param and `String` return
pub fn shout(s: &str) -> String {
    s.to_uppercase()
}

// primitive by mut ref -> skipped, warning emitted
pub fn inc(x: &mut f64) {
    *x += 1.0;
}

// primitive by ref -> skipped, warning emitted
pub fn read_ref(x: &f64) -> f64 {
    *x
}

// struct by mut ref -> mutation propagates, no clone
pub fn nudge(p: &mut Point, by: f64) {
    p.x += by;
    p.y += by;
}

// struct by value -> cloned at the boundary, original is untouched
pub fn shift_copy(mut p: Point, by: f64) -> Point {
    p.x += by;
    p
}

// returns a struct as a live handle
pub fn origin() -> Point {
    Point::new(0.0, 0.0)
}
