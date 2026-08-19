//! Codegen layer

// pub mod check;
pub mod codegen;
pub mod db;
pub mod diag;
pub mod inputs;
mod ir;
mod mangle;

#[cfg(test)]
pub(crate) mod test;
