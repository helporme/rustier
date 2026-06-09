use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct Span {
    /// `filename` interpretation depends on the originating crate: relative to
    /// the project root for the local crate, absolute under `~/.cargo/...` for
    /// registry deps, and so on. `None` if there is no source span available.
    pub filename: PathBuf,
    /// 1-indexed `(line, column)` begin/end of the span.
    pub begin: (usize, usize),
    /// 1-indexed `(line, column)` begin/end of the span.
    pub end: (usize, usize),
}
