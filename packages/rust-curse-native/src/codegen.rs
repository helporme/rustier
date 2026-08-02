//! Codegen layer

pub mod api;
pub mod bindings;
pub mod cast;
pub mod check;
pub mod db;
pub mod diag;
pub mod fragment;
pub mod ir;
pub mod js;
mod mangle;
pub mod rustdoc;
pub mod ts;

#[cfg(test)]
#[doc(hidden)]
pub mod test;
