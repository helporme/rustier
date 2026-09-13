use crate::compiler::{db::Db, ir::ffi::Ffi};

const LOADER: &str = include_str!("napi-loader.mjs");

impl<'db> Ffi<'db> {
    /// Selects a sibling addon at runtime and re-exports the generated shim's
    /// FFI API.
    pub fn render_napi_ffi(self, db: &'db dyn Db) -> String {
        let mut js = LOADER.to_owned();
        for symbol in self.symbols(db) {
            js.push_str(&format!("export const {symbol} = native.{symbol};\n"));
        }
        js
    }
}
