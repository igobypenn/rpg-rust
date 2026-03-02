use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(NonZeroUsize);

impl NodeId {
    /// Creates a new NodeId from an index.
    #[inline]
    #[must_use = "NodeId must be used"]
    pub fn new(index: usize) -> Self {
        Self(NonZeroUsize::new(index + 1).expect("index + 1 should never be zero"))
    }

    /// Returns the underlying index.
    #[inline]
    #[must_use = "index should be used"]
    pub fn index(&self) -> usize {
        self.0.get() - 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeId(NonZeroUsize);

impl EdgeId {
    /// Creates a new EdgeId from an index.
    #[inline]
    #[must_use = "EdgeId must be used"]
    pub fn new(index: usize) -> Self {
        Self(NonZeroUsize::new(index + 1).expect("index + 1 should never be zero"))
    }

    /// Returns the underlying index.
    #[inline]
    #[must_use = "index should be used"]
    pub fn index(&self) -> usize {
        self.0.get() - 1
    }
}
