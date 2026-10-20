mod cast;
mod class_templates;
mod classes;
mod functions;
mod imports;
mod modules;
mod traits;

#[cfg(test)]
mod test;

/// Output paths used for a single JavaScript code generation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct JsCodegenPaths {
    /// Module specifier exposing all ABI functions.
    pub abi_js_mod_id: String,
}
