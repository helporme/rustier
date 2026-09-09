//! Incremental compiler database and intermediate representations.

pub mod db;
pub mod diag;
pub mod inputs;
pub mod ir;
pub mod path;

// mod check;
pub mod codegen;
mod mangle;

#[cfg(test)]
mod test;
