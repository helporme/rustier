//! Compiles rustdoc into TypeScript declaration source maps.

mod enum_;
mod fn_;
mod mappings;
mod mod_;
mod struct_;
#[cfg(test)]
mod test;

pub use mappings::Mapping;
pub use mod_::{SourceMap, compile_mod};
