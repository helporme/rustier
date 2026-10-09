//@ api: node
//@ cargo-check

#[derive(Clone)]
#[non_exhaustive]
pub enum NonExhaustiveEnum {
    Unit,
    Tuple(u32),
    Struct { value: String },
}
