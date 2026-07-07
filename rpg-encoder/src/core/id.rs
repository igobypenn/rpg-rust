//! Node identifier type.

use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;

/// A stable, unique identifier for a node in the graph.
///
/// Internally backed by `NonZeroUsize` for niche optimization (Option<NodeId>
/// is the same size as NodeId). The `index()` method returns the 0-based
/// position; `new(0)` creates the first valid id.
///
/// NodeIds are stable across save/reload cycles when using
/// [`RpgGraph::add_node_preserving_id`](crate::RpgGraph::add_node_preserving_id).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(NonZeroUsize);

impl NodeId {
    /// Create a NodeId from a 0-based index.
    ///
    /// # Panics
    /// Panics if `index` is `usize::MAX` (would overflow the internal NonZeroUsize).
    #[inline]
    #[must_use = "NodeId must be used"]
    pub fn new(index: usize) -> Self {
        Self(NonZeroUsize::new(index + 1).expect("index + 1 should never be zero"))
    }

    /// Return the 0-based index of this id.
    #[inline]
    #[must_use = "index should be used"]
    pub fn index(&self) -> usize {
        self.0.get() - 1
    }
}
