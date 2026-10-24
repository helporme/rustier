#[salsa::input]
pub struct JsFile {
    #[returns(ref)]
    pub module_id: String,
    #[returns(ref)]
    pub contents: String,
}

#[salsa::interned]
pub struct RustFunction<'db> {
    #[returns(ref)]
    pub name: String,
    pub return_type: RustType<'db>,
    #[returns(ref)]
    pub body: Vec<Part<'db>>,
}

#[salsa::interned(debug)]
pub struct RustType<'db> {
    #[returns(ref)]
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum Part<'db> {
    Text(String),
    Interpolation { rust_type: RustType<'db> },
}
