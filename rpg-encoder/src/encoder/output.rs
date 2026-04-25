use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::RpgGraph;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedNode {
    pub id: String,
    pub category: String,
    pub kind: String,
    pub language: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<SerializedLocation>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SerializedSourceRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_feature: Option<String>,
    #[serde(default, skip_serializing_if = "is_default_node_level")]
    pub node_level: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedLocation {
    pub file: String,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedEdge {
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub edge_type: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphMetadata {
    pub version: String,
    pub languages: Vec<String>,
    pub node_count: usize,
    pub edge_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedSourceRef {
    pub start_line: usize,
    pub end_line: usize,
}

fn is_default_node_level(level: &str) -> bool {
    level == "low"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SerializedGraph {
    pub nodes: Vec<SerializedNode>,
    pub edges: Vec<SerializedEdge>,
    pub metadata: GraphMetadata,
}

impl From<&crate::core::SourceLocation> for SerializedLocation {
    fn from(loc: &crate::core::SourceLocation) -> Self {
        Self {
            file: loc.file.to_string_lossy().to_string(),
            start_line: loc.start_line,
            start_column: loc.start_column,
            end_line: loc.end_line,
            end_column: loc.end_column,
        }
    }
}

pub fn serialize_graph(graph: &RpgGraph) -> SerializedGraph {
    let languages: Vec<String> = graph
        .nodes()
        .filter(|n| !n.language.is_empty())
        .map(|n| n.language.clone())
        .filter(|l| !l.is_empty())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let nodes: Vec<SerializedNode> = graph
        .nodes()
        .map(|n| SerializedNode {
            id: format!("node_{}", n.id.index()),
            category: format!("{:?}", n.category).to_lowercase(),
            kind: n.kind.clone(),
            language: n.language.clone(),
            name: n.name.clone(),
            path: n.path.as_ref().map(|p| p.to_string_lossy().to_string()),
            location: n.location.as_ref().map(|l| l.into()),
            metadata: n.metadata.clone(),
            description: n.description.clone(),
            features: n.features.clone(),
            feature_path: n.feature_path.clone(),
            signature: n.signature.clone(),
            source_ref: n.source_ref.as_ref().map(|sr| SerializedSourceRef {
                start_line: sr.start_line,
                end_line: sr.end_line,
            }),
            semantic_feature: n.semantic_feature.clone(),
            node_level: format!("{:?}", n.node_level).to_lowercase(),
            documentation: n.documentation.clone(),
        })
        .collect();

    let edges: Vec<SerializedEdge> = graph
        .edges()
        .map(|(s, t, e)| SerializedEdge {
            source: format!("node_{}", s.index()),
            target: format!("node_{}", t.index()),
            edge_type: format!("{:?}", e.edge_type).to_lowercase(),
            metadata: e.metadata.clone(),
        })
        .collect();

    let node_count = nodes.len();
    let edge_count = edges.len();

    SerializedGraph {
        nodes,
        edges,
        metadata: GraphMetadata {
            version: env!("CARGO_PKG_VERSION").to_string(),
            languages,
            node_count,
            edge_count,
        },
    }
}

pub fn to_json(graph: &RpgGraph) -> crate::error::Result<String> {
    let serialized = serialize_graph(graph);
    Ok(serde_json::to_string_pretty(&serialized)?)
}

pub fn to_json_compact(graph: &RpgGraph) -> crate::error::Result<String> {
    let serialized = serialize_graph(graph);
    Ok(serde_json::to_string(&serialized)?)
}
