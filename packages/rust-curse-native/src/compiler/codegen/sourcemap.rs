//! Generates Source Map v3 documents for TypeScript declarations.

// The sourcemap generation code is admitedly heavily ai assisted (unlike the
// rest of the project which is hand written) since it's not the most important
// part of the project and my lack of time and interest in it.

mod classes;
mod functions;
mod mappings;
mod modules;
#[cfg(test)]
mod test;
