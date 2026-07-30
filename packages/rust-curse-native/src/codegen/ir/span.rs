use std::path::PathBuf;

/// A source code span as one-based row/column bounds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Span {
    pub filename: PathBuf,
    pub begin: (usize, usize),
    pub end: (usize, usize),
}
