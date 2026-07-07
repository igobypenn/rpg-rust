//! Edge types for the Repository Planning Graph.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of relationships between nodes in the RPG graph.
///
/// Each variant corresponds to a semantic relationship discovered during
/// parsing or LLM enrichment. The edge direction matters: `Calls` goes
/// from caller → callee; `Contains` goes from parent → child.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    /// Parent → child structural containment (Repository → Directory → File → Function).
    Contains,
    /// File → module/file that it imports from.
    Imports,
    /// Caller function → callee function.
    Calls,
    /// Child type → parent type (inheritance).
    Extends,
    /// Implementor → trait/interface.
    Implements,
    /// Symbol → referenced definition (non-call reference, e.g. type annotation).
    References,
    /// General dependency (file-level or module-level).
    DependsOn,
    /// Language → native function bound via FFI (unique to rpg-mcp).
    FfiBinding,
    /// Definition → symbol it defines.
    Defines,
    /// General usage edge.
    Uses,
    /// Function/variable → type it references (type annotations, generics).
    UsesType,
    /// Node → feature it implements.
    ImplementsFeature,
    /// V^L node → V^H functional centroid (membership in a behavioral area).
    BelongsToFeature,
    /// Feature → sub-feature (hierarchical feature containment).
    ContainsFeature,
    /// Node → component it belongs to.
    BelongsToComponent,
}

/// A directed edge between two nodes in the graph.
///
/// Edges carry an [`EdgeType`] and optional metadata (e.g. `"receiver"` for
/// method calls, `"source": "scip"` for SCIP-sourced edges).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// The semantic type of this edge.
    pub edge_type: EdgeType,
    /// Optional key-value metadata (e.g. receiver name, call kind, provenance).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Edge {
    /// Create a new edge with no metadata.
    #[must_use = "Edge must be used"]
    pub fn new(edge_type: EdgeType) -> Self {
        Self {
            edge_type,
            metadata: HashMap::new(),
        }
    }

    /// Add a metadata key-value pair to this edge.
    #[must_use = "Edge must be used"]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

impl From<EdgeType> for Edge {
    fn from(edge_type: EdgeType) -> Self {
        Self::new(edge_type)
    }
}

impl std::fmt::Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeType::Contains => write!(f, "contains"),
            EdgeType::Imports => write!(f, "imports"),
            EdgeType::Calls => write!(f, "calls"),
            EdgeType::Extends => write!(f, "extends"),
            EdgeType::Implements => write!(f, "implements"),
            EdgeType::References => write!(f, "references"),
            EdgeType::DependsOn => write!(f, "depends_on"),
            EdgeType::FfiBinding => write!(f, "ffi_binding"),
            EdgeType::Defines => write!(f, "defines"),
            EdgeType::Uses => write!(f, "uses"),
            EdgeType::UsesType => write!(f, "uses_type"),
            EdgeType::ImplementsFeature => write!(f, "implements_feature"),
            EdgeType::BelongsToFeature => write!(f, "belongs_to_feature"),
            EdgeType::ContainsFeature => write!(f, "contains_feature"),
            EdgeType::BelongsToComponent => write!(f, "belongs_to_component"),
        }
    }
}
