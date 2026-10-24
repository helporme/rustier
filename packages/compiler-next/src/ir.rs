#[salsa::input]
pub struct JsFile {
    #[returns(ref)]
    pub module_id: String,
    #[returns(ref)]
    pub contents: String,
}

#[salsa::interned(debug)]
pub struct RustType<'db> {
    #[returns(ref)]
    pub source: String,
}
