//! Codegen layer

mod bir;
pub mod check;
pub mod codegen;
pub mod db;
pub mod diag;
mod ir;
pub mod lower;
mod mangle;
mod rust_ir;
pub mod rustdoc;

#[cfg(test)]
#[doc(hidden)]
pub mod test;
