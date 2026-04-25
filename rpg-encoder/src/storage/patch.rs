use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::encoder::{SerializedEdge, SerializedNode};

/// An edge removed by a patch, identified by its endpoints and type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedEdge {
    /// Source node ID (e.g. `"node_0"`).
    pub source: String,
    /// Target node ID (e.g. `"node_5"`).
    pub target: String,
    /// Edge type (e.g. `"calls"`, `"imports"`).
    #[serde(rename = "type")]
    pub edge_type: String,
}

/// Per-file changes within a patch: nodes and edges added/removed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePatch {
    /// Content hash before the change.
    pub old_hash: String,
    /// Content hash after the change.
    pub new_hash: String,
    /// Node IDs to remove from the graph.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_node_ids: Vec<String>,
    /// Full node data to add to the graph.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_nodes: Vec<SerializedNode>,
    /// Edges to remove from the graph.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_edges: Vec<RemovedEdge>,
    /// Edges to add to the graph.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_edges: Vec<SerializedEdge>,
}

/// Top-level change set for a single patch.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PatchChanges {
    /// Files added since the last base or patch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_files: Vec<PathBuf>,
    /// Files deleted since the last base or patch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deleted_files: Vec<PathBuf>,
    /// Per-file node/edge diffs for modified files.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub modified_files: HashMap<String, FilePatch>,
}

/// Counts of entities changed in a patch.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PatchStats {
    #[serde(default)]
    pub files_added: usize,
    #[serde(default)]
    pub files_deleted: usize,
    #[serde(default)]
    pub files_modified: usize,
    #[serde(default)]
    pub nodes_added: usize,
    #[serde(default)]
    pub nodes_removed: usize,
    #[serde(default)]
    pub edges_added: usize,
    #[serde(default)]
    pub edges_removed: usize,
}

/// A single patch recording all graph changes from one update cycle.
///
/// Patches are global (cover all file changes) and form a chain via `parent_seq`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    /// Sequence number for this patch.
    pub seq: u32,
    /// Unix timestamp when the patch was created.
    pub timestamp: u64,
    /// Sequence number of the parent patch (0 for the first patch).
    pub parent_seq: u32,
    /// The actual changes.
    pub changes: PatchChanges,
    /// Summary counts.
    pub stats: PatchStats,
}

impl Patch {
    /// Create a new patch with auto-set timestamp.
    #[must_use]
    pub fn new(seq: u32, parent_seq: u32) -> Self {
        Self {
            seq,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            parent_seq,
            changes: PatchChanges::default(),
            stats: PatchStats::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: &str, name: &str) -> SerializedNode {
        SerializedNode {
            id: id.to_string(),
            category: "function".to_string(),
            kind: "function".to_string(),
            language: "rust".to_string(),
            name: name.to_string(),
            path: Some("src/main.rs".to_string()),
            location: None,
            metadata: HashMap::new(),
            description: None,
            features: vec![],
            feature_path: None,
            signature: None,
            source_ref: None,
            semantic_feature: None,
            node_level: "low".to_string(),
            documentation: None,
        }
    }

    #[test]
    fn test_patch_serialize_roundtrip() {
        let mut patch = Patch::new(1, 0);
        patch.changes.added_files.push(PathBuf::from("src/new.rs"));
        patch
            .changes
            .deleted_files
            .push(PathBuf::from("src/old.rs"));
        patch.changes.modified_files.insert(
            "src/main.rs".to_string(),
            FilePatch {
                old_hash: "sha256:aaa".to_string(),
                new_hash: "sha256:bbb".to_string(),
                removed_node_ids: vec!["node_5".to_string()],
                added_nodes: vec![make_node("node_10", "new_fn")],
                removed_edges: vec![RemovedEdge {
                    source: "node_0".to_string(),
                    target: "node_5".to_string(),
                    edge_type: "calls".to_string(),
                }],
                added_edges: vec![SerializedEdge {
                    source: "node_0".to_string(),
                    target: "node_10".to_string(),
                    edge_type: "calls".to_string(),
                    metadata: HashMap::new(),
                }],
            },
        );
        patch.stats.files_added = 1;
        patch.stats.files_deleted = 1;
        patch.stats.files_modified = 1;
        patch.stats.nodes_added = 1;
        patch.stats.nodes_removed = 1;

        let json = serde_json::to_string(&patch).unwrap();
        let loaded: Patch = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.seq, 1);
        assert_eq!(loaded.parent_seq, 0);
        assert_eq!(loaded.changes.added_files.len(), 1);
        assert_eq!(loaded.changes.deleted_files.len(), 1);
        assert_eq!(loaded.changes.modified_files.len(), 1);
        assert_eq!(loaded.stats.nodes_added, 1);
    }

    #[test]
    fn test_empty_patch_serializes_cleanly() {
        let patch = Patch::new(1, 0);
        let json = serde_json::to_string(&patch).unwrap();

        assert!(!json.contains("added_files"));
        assert!(!json.contains("deleted_files"));
        assert!(!json.contains("removed_node_ids"));
        assert!(!json.contains("added_nodes"));
    }

    #[test]
    fn test_patch_new_auto_timestamp() {
        let patch = Patch::new(1, 0);
        assert!(patch.timestamp > 0);
        assert_eq!(patch.seq, 1);
        assert_eq!(patch.parent_seq, 0);
    }

    #[test]
    fn test_patch_stats_default() {
        let stats = PatchStats::default();
        assert_eq!(stats.files_added, 0);
        assert_eq!(stats.nodes_added, 0);
    }
}
