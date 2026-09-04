//! Incremental compiler database and intermediate representations.

pub mod db;
pub mod diag;
pub mod inputs;
pub mod ir;
pub mod path;
pub mod workspace;

// mod check;
mod codegen;
mod mangle;

#[cfg(test)]
mod test;
