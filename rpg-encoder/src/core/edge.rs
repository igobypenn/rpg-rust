use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    Contains,
    Imports,
    Calls,
    Extends,
    Implements,
    References,
    DependsOn,
    FfiBinding,
    Defines,
    Uses,
    UsesType,
    ImplementsFeature,
    BelongsToFeature,
    ContainsFeature,
    BelongsToComponent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub edge_type: EdgeType,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Edge {
    #[must_use = "Edge must be used"]
    pub fn new(edge_type: EdgeType) -> Self {
        Self {
            edge_type,
            metadata: HashMap::new(),
        }
    }

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

/// Classification of edge types into functional or dependency views.
/// 
/// Per the paper, edges are classified as:
/// - E_dep (Dependency): Calls, Imports, DependsOn, etc.
/// - E_feature (Functional): BelongsToFeature, ImplementsFeature, ContainsFeature
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeView {
    /// Functional hierarchy edges (E_feature)
    Functional,
    /// Dependency edges (E_dep)
    Dependency,
}

impl EdgeType {
    /// Returns the view classification for this edge type.
    /// 
    /// - Functional edges: BelongsToFeature, ImplementsFeature, ContainsFeature
    /// - dependency edges: all others (Calls, Imports, etc.)
    #[must_use]
    pub fn view(&self) -> EdgeView {
        match self {
            EdgeType::BelongsToFeature 
            | EdgeType::ImplementsFeature 
            | EdgeType::ContainsFeature => EdgeView::Functional,
            _ => EdgeView::Dependency,
        }
    }
}
