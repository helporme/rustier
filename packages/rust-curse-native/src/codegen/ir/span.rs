use std::path::PathBuf;

/// A source code span as one-based row/column bounds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Span {
    // note: split on two paths is a preparation for supporting external crates, however currently
    // it doesn't make much sense.
    /// The absolute path to the source file.
    pub abs_path: PathBuf,
    /// The path rechable from the crate root.
    ///
    /// It might be a relative path if the span is pointing to the file inside
    /// the "main" crate, or an absolute path otherwise.
    pub src_path: PathBuf,
    pub begin: (usize, usize),
    pub end: (usize, usize),
}
