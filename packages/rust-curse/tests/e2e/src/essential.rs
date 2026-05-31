use std::fmt;
use std::ops::Add;

/// A point in 2D space.
#[derive(Clone)]
pub struct Point {
    /// Horizontal position.
    pub x: f64,
    /// Vertical position.
    pub y: f64,
}

#[derive(Clone)]
pub struct Line {
    pub start: Point,
    pub end: Point,
}

#[derive(Clone)]
pub struct Triangle {
    pub line: Line,
    pub point: Point,
}

#[derive(Clone)]
pub struct Quad {
    pub tri: Triangle,
}

#[derive(Clone)]
pub struct Other {
    pub value: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn length(&self) -> f64 {
        (self.x.powi(2) + self.y.powi(2)).sqrt()
    }

    pub fn scale(&mut self, factor: f64) {
        self.x *= factor;
        self.y *= factor;
    }

    pub fn add_assign(&mut self, other: &Point) {
        self.x += other.x;
        self.y += other.y;
    }

    pub fn consume(self) -> Self {
        self
    }

    pub fn to_origin(&self) -> Other {
        Other {
            value: self.length(),
        }
    }

    pub fn combine(&self, a: &Point, b: &Point) -> Self {
        Self {
            x: self.x + a.x + b.x,
            y: self.y + a.y + b.y,
        }
    }
}

impl Point {
    #[allow(dead_code)]
    pub(crate) fn translate(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
    }
}

impl Line {
    pub fn from(start: Point, end: Point) -> Self {
        Self { start, end }
    }

    pub fn of(point: Point) -> Self {
        Self {
            start: point.clone(),
            end: point,
        }
    }
}

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

#[derive(Clone)]
pub struct Bigs {
    pub signed: i64,
}

pub fn tick() {}

#[derive(Clone)]
pub struct SelfNew {
    pub v: f64,
}

impl SelfNew {
    pub fn new(&self) -> Self {
        Self { v: self.v }
    }
}

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

#[derive(Clone)]
pub struct StaticNew {
    pub v: f64,
}

impl StaticNew {
    pub fn new(x: f64) -> f64 {
        x * 2.0
    }
}

#[derive(Clone)]
pub struct Zero {
    pub value: f64,
}

impl Zero {
    pub fn new() -> Self {
        Self { value: 0.0 }
    }
}

#[derive(Clone)]
pub struct TwoNew {
    pub v: f64,
}

impl TwoNew {
    pub fn new(v: f64) -> Self {
        Self { v }
    }
}

pub trait TwoNewAlt {
    fn new(v: f64) -> TwoNew;
}

impl TwoNewAlt for TwoNew {
    fn new(v: f64) -> TwoNew {
        TwoNew { v: v + 100.0 }
    }
}

#[derive(Clone)]
pub struct Span {
    pub start: Point,
    pub end: Point,
}

impl Span {
    pub fn new(start: Point, end: Point) -> Self {
        Self { start, end }
    }
}

#[derive(Clone)]
pub struct Maker {
    pub v: f64,
}

impl Maker {
    pub fn create(v: f64) -> Self {
        Self { v }
    }
}

#[derive(Clone)]
pub struct Both {
    pub value: f64,
}

impl Both {
    pub fn new() -> Self {
        Self { value: 42.0 }
    }
}

pub struct NoClone {
    pub v: f64,
}

impl NoClone {
    pub fn new(v: f64) -> Self {
        Self { v }
    }

    pub fn bump(&mut self, by: f64) {
        self.v += by;
    }

    pub fn consume(self) -> f64 {
        self.v
    }
}

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

#[derive(Clone)]
pub struct Builder {
    pub v: f64,
}

impl Builder {
    pub fn new(v: f64) -> Self {
        Self { v }
    }

    pub fn with(self, by: f64) -> Self {
        Self { v: self.v + by }
    }

    pub fn set(&mut self, v: f64) {
        self.v = v;
    }
}

#[derive(Clone)]
pub struct Shadow {
    pub len: f64,
}

impl Shadow {
    pub fn len(&self) -> f64 {
        self.len
    }
}

#[derive(Clone)]
pub struct Hazard {
    pub value: f64,
    pub then: f64,
}

impl Hazard {
    #[allow(non_snake_case)]
    pub fn toString(&self) -> String {
        format!("Hazard({}, {})", self.value, self.then)
    }

    #[allow(non_snake_case)]
    pub fn valueOf(&self) -> f64 {
        self.value + self.then
    }
}

pub struct Reenter {
    pub value: f64,
}

impl Reenter {
    pub fn new(value: f64) -> Self {
        Self { value }
    }

    pub fn outer(&mut self) {
        self.value += 1.0;
    }
}

pub struct BadField {
    pub items: Vec<f64>,
}

pub enum Color {
    Red,
    Blue,
}

pub fn bad_return() -> Vec<f64> {
    Vec::new()
}

pub struct Marker;

pub struct Pair(pub f64, pub f64);

pub struct Holder {
    pub mark: Marker,
}

pub fn use_pair(p: Pair) -> f64 {
    p.0
}

pub fn make_pair() -> Pair {
    Pair(1.0, 2.0)
}

pub struct Generic<T> {
    pub value: T,
}

pub fn id<T>(value: T) -> T {
    value
}

pub struct SliceField<'a> {
    pub items: &'a [f64],
}

pub struct ArrayField {
    pub items: [f64; 2],
}

pub struct TupleField {
    pub items: (f64, f64),
}

pub use std::collections::HashMap as ExternTy;

pub struct Extern {
    pub x: ExternTy<String, String>,
}

#[allow(dead_code)]
fn private_fn() -> f64 {
    42.0
}

pub fn add(a: f64, b: f64) -> f64 {
    a + b
}

pub fn shout(s: &str) -> String {
    s.to_uppercase()
}

pub fn inc(x: &mut f64) {
    *x += 1.0;
}

pub fn read_ref(x: &f64) -> f64 {
    *x
}

pub fn nudge(p: &mut Point, by: f64) {
    p.x += by;
    p.y += by;
}

pub fn shift_copy(mut p: Point, by: f64) -> Point {
    p.x += by;
    p
}

pub fn origin() -> Point {
    Point::new(0.0, 0.0)
}

pub fn append(s: &mut String) {
    s.push('!');
}

pub fn label(p: &mut Point, label: String) -> String {
    p.x += label.len() as f64;
    label
}

pub fn many(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> f64 {
    a + b * 10.0 + c * 100.0 + d * 1000.0 + e * 10000.0 + f * 100000.0
}

pub fn boom() {
    panic!("boom from Rust");
}

pub fn take_noclone(value: NoClone) -> f64 {
    value.v
}

#[derive(Clone)]
pub struct NewAndOf {
    pub value: f64,
}

impl NewAndOf {
    pub fn new(value: f64) -> Self {
        Self { value }
    }

    pub fn of(value: f64) -> f64 {
        value + 1.0
    }
}

#[derive(Clone)]
pub struct Keywords {
    pub r#class: f64,
    pub r#delete: f64,
}

#[derive(Clone)]
pub struct SnakeCase {
    pub my_field: f64,
}

pub(crate) fn restricted() -> f64 {
    1.0
}

#[derive(Clone)]
pub struct Named {
    pub v: f64,
}

impl fmt::Display for Named {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.v)
    }
}

#[derive(Clone)]
pub struct Adder {
    pub v: f64,
}

impl Add for Adder {
    type Output = Adder;

    fn add(self, rhs: Adder) -> Self::Output {
        Adder { v: self.v + rhs.v }
    }
}

pub struct ManualClone {
    pub v: f64,
}

impl Clone for ManualClone {
    fn clone(&self) -> Self {
        Self { v: self.v }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DerivedExtras {
    pub v: f64,
}

#[derive(Clone, Default)]
pub struct Config {
    pub enabled: bool,
}

#[derive(Clone)]
pub struct Empty {}

#[derive(Clone)]
pub struct One {
    pub v: f64,
}

pub struct Mixed {
    pub a: f64,
    b: f64,
}

impl Mixed {
    pub fn new(a: f64, b: f64) -> Self {
        Self { a, b }
    }

    pub fn sum(&self) -> f64 {
        self.a + self.b
    }
}

pub struct Opaque {
    value: f64,
}

impl Opaque {
    pub fn new(value: f64) -> Self {
        Self { value }
    }

    pub fn value(&self) -> f64 {
        self.value
    }
}

pub fn point() -> f64 {
    12.0
}

#[deprecated(note = "use add instead")]
pub fn old() -> f64 {
    0.0
}

#[derive(Clone)]
pub enum BuildOutput {
    Wasm { wasm_abs_path: String },
    Node { node_abs_path: String },
    Skipped,
}

#[derive(Clone)]
pub enum UnitMix {
    Full { x: f64 },
    Empty {},
    Unit,
}

#[derive(Clone)]
pub enum CamelTags {
    HttpGet { code: f64 },
    Done,
}

#[derive(Clone)]
pub enum SharedOk {
    A { data: f64 },
    B { data: f64 },
    C,
}

#[derive(Clone)]
pub enum SharedBad {
    A { data: f64 },
    B { data: String },
}

#[derive(Clone)]
pub enum BadTuple {
    A(f64),
    B { x: f64 },
}

pub enum Uninhabited {}

#[derive(Clone)]
#[allow(non_camel_case_types)]
pub enum TagCollision {
    URL { a: f64 },
    uRL { b: f64 },
}

#[derive(Clone)]
pub enum Event {
    Click { r#type: String, x: f64 },
    Key { code: f64 },
}

#[derive(Clone)]
pub enum Element {
    Tag { id: f64 },
    Plain,
}

impl Element {
    pub fn r#type(&self) -> f64 {
        match self {
            Element::Tag { id } => *id,
            Element::Plain => 0.0,
        }
    }
}

#[derive(Clone)]
pub struct Mesh {
    pub x: f64,
}

#[derive(Clone)]
pub enum Hitbox {
    Active { mesh: Mesh },
    Disabled,
}

#[derive(Clone)]
pub enum Inner {
    On { level: f64 },
    Off,
}

impl Inner {
    pub fn on(level: f64) -> Self {
        Inner::On { level }
    }

    pub fn level_or_zero(&self) -> f64 {
        match self {
            Inner::On { level } => *level,
            Inner::Off => 0.0,
        }
    }

    pub fn set_level(&mut self, v: f64) {
        if let Inner::On { level } = self {
            *level = v;
        }
    }

    pub fn switch_off(&mut self) {
        *self = Inner::Off;
    }
}

pub fn force_off(i: &mut Inner) {
    *i = Inner::Off;
}

pub fn use_inner(i: Inner) -> f64 {
    i.level_or_zero()
}

#[derive(Clone)]
pub enum Outer {
    Has { inner: Inner },
    None,
}

#[derive(Clone)]
pub enum EnumWithOf {
    A { v: f64 },
    B,
}

impl EnumWithOf {
    pub fn of(v: f64) -> f64 {
        v + 1.0
    }
}

#[derive(Clone)]
pub enum EnumWithNew {
    Active { level: f64 },
    Idle,
}

impl EnumWithNew {
    pub fn new(level: f64) -> Self {
        EnumWithNew::Active { level }
    }
}

pub static ENUM_CLONES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub enum CloneCounted {
    Some { v: f64 },
    Nil,
}

impl Clone for CloneCounted {
    fn clone(&self) -> Self {
        ENUM_CLONES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        match self {
            CloneCounted::Some { v } => CloneCounted::Some { v: *v },
            CloneCounted::Nil => CloneCounted::Nil,
        }
    }
}

impl CloneCounted {
    pub fn new(v: f64) -> Self {
        CloneCounted::Some { v }
    }

    pub fn value(&self) -> f64 {
        match self {
            CloneCounted::Some { v } => *v,
            CloneCounted::Nil => 0.0,
        }
    }

    pub fn bump(&mut self, by: f64) {
        if let CloneCounted::Some { v } = self {
            *v += by;
        }
    }
}

pub fn reset_enum_clones() {
    ENUM_CLONES.store(0, std::sync::atomic::Ordering::SeqCst);
}

pub fn enum_clone_count() -> f64 {
    ENUM_CLONES.load(std::sync::atomic::Ordering::SeqCst) as f64
}

pub fn read_cc(c: &CloneCounted) -> f64 {
    c.value()
}

pub fn bump_cc(c: &mut CloneCounted, by: f64) {
    c.bump(by);
}

pub fn take_cc(c: CloneCounted) -> f64 {
    c.value()
}

pub fn make_cc(v: f64) -> CloneCounted {
    CloneCounted::Some { v }
}

pub enum NonCloneState {
    A { v: f64 },
    B,
}

impl NonCloneState {
    pub fn new(v: f64) -> Self {
        NonCloneState::A { v }
    }

    pub fn value(&self) -> f64 {
        match self {
            NonCloneState::A { v } => *v,
            NonCloneState::B => 0.0,
        }
    }

    pub fn into_value(self) -> f64 {
        self.value()
    }
}

pub fn take_state(s: NonCloneState) -> f64 {
    s.value()
}

pub struct UsesColor {
    pub c: Color,
}
