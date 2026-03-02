//! Operations Layer (Paper Phase 4).
//!
//! Implements unified tools for graph traversal and semantic search:
//! - `SearchNode`: Semantic search for nodes by feature/description similarity
//! - `FetchNode`: Retrieve detailed node information
//! - `ExploreRPG`: Graph traversal with filtering

mod search;
mod fetch;
mod explore;

pub use search::{SearchNode, SearchResult, SearchConfig};
pub use fetch::{FetchNode, FetchResult, NodeDetail};
pub use explore::{ExploreRPG, ExploreResult, ExploreFilter, TraversalDirection};
