//! Source location types.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A precise location in a source file (file + line/column range).
///
/// Used by [`Node::location`](crate::Node::location) for fine-grained
/// position data. Line numbers are 1-based; columns are 0-based byte offsets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    /// Relative file path (repo-relative).
    pub file: PathBuf,
    /// Starting line number (1-based).
    pub start_line: usize,
    /// Starting column (0-based byte offset).
    pub start_column: usize,
    /// Ending line number (1-based, inclusive).
    pub end_line: usize,
    /// Ending column (0-based byte offset).
    pub end_column: usize,
}

impl SourceLocation {
    /// Create a new source location.
    #[must_use = "SourceLocation must be used"]
    pub fn new(
        file: PathBuf,
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
    ) -> Self {
        Self {
            file,
            start_line,
            start_column,
            end_line,
            end_column,
        }
    }

    /// Create a single-line source location.
    #[must_use = "SourceLocation must be used"]
    pub fn single_line(file: PathBuf, line: usize, start_col: usize, end_col: usize) -> Self {
        Self::new(file, line, start_col, line, end_col)
    }
}
