mod cast;
mod class_factories;
mod classes;
mod functions;
mod imports;
mod modules;

#[cfg(test)]
mod test;

/// Output paths used for a single JavaScript code generation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct JsCodegenPaths {
    /// Module specifier exposing all FFI functions.
    pub ffi_js_mod_id: String,
}
