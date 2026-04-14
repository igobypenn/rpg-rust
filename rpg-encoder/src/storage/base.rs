use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::{Edge, EdgeType, Node, NodeCategory, NodeId, RpgGraph};
use crate::encoder::{serialize_graph, SerializedGraph};
use crate::incremental::{CachedUnit, RpgSnapshot, SNAPSHOT_VERSION};

/// Serialized form of an RPG snapshot, combining the graph with file hashes and unit cache.
///
/// This is what gets written to `base.json`.
#[derive(Debug, Serialize, Deserialize)]
pub struct BaseSnapshot {
    /// The graph structure (nodes + edges + metadata).
    #[serde(flatten)]
    pub graph: SerializedGraph,
    /// Per-file content hashes for incremental diff detection.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub file_hashes: HashMap<String, String>,
    /// Per-file unit caches (parsed code units with their content hashes).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub unit_cache: HashMap<String, Vec<CachedUnit>>,
    /// Snapshot format version.
    #[serde(default)]
    pub snapshot_version: u32,
}

impl BaseSnapshot {
    /// Create a `BaseSnapshot` from an in-memory `RpgSnapshot`.
    #[must_use]
    pub fn from_snapshot(snapshot: &RpgSnapshot) -> Self {
        let graph = serialize_graph(&snapshot.graph);

        let file_hashes: HashMap<String, String> = snapshot
            .file_hashes
            .iter()
            .map(|(p, h)| (p.to_string_lossy().to_string(), h.clone()))
            .collect();

        let unit_cache: HashMap<String, Vec<CachedUnit>> = snapshot
            .unit_cache
            .iter()
            .map(|(p, units)| (p.to_string_lossy().to_string(), units.clone()))
            .collect();

        Self {
            graph,
            file_hashes,
            unit_cache,
            snapshot_version: SNAPSHOT_VERSION,
        }
    }

    /// Convert back into an in-memory `RpgSnapshot`.
    #[must_use]
    pub fn into_snapshot(self, repo_dir: &Path, repo_name: &str) -> RpgSnapshot {
        let mut rpg_graph = RpgGraph::new();
        let mut id_map: HashMap<String, NodeId> = HashMap::new();

        for node_data in &self.graph.nodes {
            let category = parse_category(&node_data.category);
            let mut node = Node::new(
                NodeId::new(0),
                category,
                &node_data.kind,
                &node_data.language,
                &node_data.name,
            );
            if let Some(ref path) = node_data.path {
                node = node.with_path(PathBuf::from(path));
            }
            let new_id = rpg_graph.add_node(node);
            id_map.insert(node_data.id.clone(), new_id);
        }

        for edge_data in &self.graph.edges {
            if let (Some(&src), Some(&tgt)) =
                (id_map.get(&edge_data.source), id_map.get(&edge_data.target))
            {
                let edge_type = parse_edge_type(&edge_data.edge_type);
                rpg_graph.add_edge(src, tgt, Edge::new(edge_type));
            }
        }

        let file_hashes: HashMap<PathBuf, String> = self
            .file_hashes
            .into_iter()
            .map(|(k, v)| (PathBuf::from(k), v))
            .collect();

        let unit_cache: HashMap<PathBuf, Vec<CachedUnit>> = self
            .unit_cache
            .into_iter()
            .map(|(k, v)| (PathBuf::from(k), v))
            .collect();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut snapshot = RpgSnapshot::new(repo_name, repo_dir);
        snapshot.graph = rpg_graph;
        snapshot.file_hashes = file_hashes;
        snapshot.unit_cache = unit_cache;
        snapshot.encoding_timestamp = now;
        snapshot.last_modified = now;
        snapshot.rebuild_node_id_index();
        snapshot
    }
}

pub(super) fn parse_category(s: &str) -> NodeCategory {
    match s {
        "repository" => NodeCategory::Repository,
        "directory" => NodeCategory::Directory,
        "file" => NodeCategory::File,
        "module" => NodeCategory::Module,
        "type" => NodeCategory::Type,
        "variable" => NodeCategory::Variable,
        "import" => NodeCategory::Import,
        "constant" => NodeCategory::Constant,
        "field" => NodeCategory::Field,
        "parameter" => NodeCategory::Parameter,
        "feature" => NodeCategory::Feature,
        "component" => NodeCategory::Component,
        "functional_centroid" => NodeCategory::FunctionalCentroid,
        _ => NodeCategory::Function,
    }
}

pub(super) fn parse_edge_type(s: &str) -> EdgeType {
    match s {
        "contains" => EdgeType::Contains,
        "imports" => EdgeType::Imports,
        "calls" => EdgeType::Calls,
        "extends" => EdgeType::Extends,
        "implements" => EdgeType::Implements,
        "depends_on" => EdgeType::DependsOn,
        "ffi_binding" => EdgeType::FfiBinding,
        "defines" => EdgeType::Defines,
        "uses" => EdgeType::Uses,
        "uses_type" => EdgeType::UsesType,
        "implements_feature" => EdgeType::ImplementsFeature,
        "belongs_to_feature" => EdgeType::BelongsToFeature,
        "contains_feature" => EdgeType::ContainsFeature,
        "belongs_to_component" => EdgeType::BelongsToComponent,
        _ => EdgeType::References,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn node_id_from_str(s: &str) -> Option<NodeId> {
    let s = s.strip_prefix("node_")?;
    let idx: usize = s.parse().ok()?;
    Some(NodeId::new(idx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_snapshot_serialize_roundtrip() {
        let mut snapshot = RpgSnapshot::new("test-repo", Path::new("/tmp/test"));
        snapshot.graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::File,
                "file",
                "rust",
                "main.rs",
            )
            .with_path(PathBuf::from("src/main.rs")),
        );

        let base = BaseSnapshot::from_snapshot(&snapshot);
        let json = serde_json::to_string(&base).unwrap();
        let loaded: BaseSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.graph.metadata.node_count, 1);
        assert_eq!(loaded.snapshot_version, SNAPSHOT_VERSION);
    }

    #[test]
    fn test_base_snapshot_empty_fields_skipped() {
        let snapshot = RpgSnapshot::new("test", Path::new("/tmp"));
        let base = BaseSnapshot::from_snapshot(&snapshot);
        let json = serde_json::to_string(&base).unwrap();

        assert!(!json.contains("file_hashes"));
        assert!(!json.contains("unit_cache"));
    }

    #[test]
    fn test_base_snapshot_into_snapshot_preserves_graph() {
        let mut snapshot = RpgSnapshot::new("test-repo", Path::new("/tmp/test"));
        let _n1 = snapshot.graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::File,
                "file",
                "rust",
                "main.rs",
            )
            .with_path(PathBuf::from("src/main.rs")),
        );

        let base = BaseSnapshot::from_snapshot(&snapshot);
        let json = serde_json::to_string(&base).unwrap();
        let loaded: BaseSnapshot = serde_json::from_str(&json).unwrap();
        let restored = loaded.into_snapshot(Path::new("/tmp/test"), "test-repo");

        assert_eq!(restored.graph.node_count(), 1);
        assert_eq!(restored.repo_name, "test-repo");
    }

    #[test]
    fn test_parse_category() {
        assert!(matches!(parse_category("file"), NodeCategory::File));
        assert!(matches!(parse_category("type"), NodeCategory::Type));
        assert!(matches!(
            parse_category("functional_centroid"),
            NodeCategory::FunctionalCentroid
        ));
        assert!(matches!(
            parse_category("unknown_value"),
            NodeCategory::Function
        ));
    }

    #[test]
    fn test_parse_edge_type() {
        assert!(matches!(parse_edge_type("calls"), EdgeType::Calls));
        assert!(matches!(parse_edge_type("contains"), EdgeType::Contains));
        assert!(matches!(
            parse_edge_type("belongs_to_feature"),
            EdgeType::BelongsToFeature
        ));
        assert!(matches!(
            parse_edge_type("unknown_type"),
            EdgeType::References
        ));
    }

    #[test]
    fn test_node_id_from_str() {
        assert!(node_id_from_str("node_0").is_some());
        assert!(node_id_from_str("node_42").is_some());
        assert!(node_id_from_str("invalid").is_none());
        assert!(node_id_from_str("").is_none());
    }
}
