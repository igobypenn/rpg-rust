//! Operations Layer (Paper Phase 4).
//!
//! Implements unified tools for graph traversal and semantic search:
//! - `SearchNode`: Semantic search for nodes by feature/description similarity
//! - `FetchNode`: Retrieve detailed node information
//! - `ExploreRPG`: Graph traversal with filtering

mod explore;
mod fetch;
mod search;

pub use explore::{ExploreFilter, ExploreRPG, ExploreResult, TraversalDirection};
pub use fetch::{FetchNode, FetchResult, NodeDetail};
pub use search::{SearchConfig, SearchNode, SearchResult};
