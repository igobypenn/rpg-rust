use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Thresholds that trigger automatic compaction of patches into a new base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionThreshold {
    /// Maximum number of unapplied patches before compaction.
    pub max_patches: usize,
    /// Trigger compaction when total patch size exceeds this ratio of base size.
    pub max_size_ratio: f64,
}

impl Default for CompactionThreshold {
    fn default() -> Self {
        Self {
            max_patches: 10,
            max_size_ratio: 0.5,
        }
    }
}

/// Metadata about the base snapshot file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseInfo {
    /// Unix timestamp when the base was written.
    pub timestamp: u64,
    /// Number of nodes in the base.
    pub node_count: usize,
    /// Number of edges in the base.
    pub edge_count: usize,
    /// SHA-256 hash of the base file for integrity verification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_hash: Option<String>,
    /// Whether the base is zstd-compressed.
    #[serde(default)]
    pub compressed: bool,
}

/// Record of a single patch in the manifest's patch list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchInfo {
    /// Monotonically increasing patch sequence number.
    pub seq: u32,
    /// Unix timestamp when the patch was written.
    pub timestamp: u64,
    /// Number of files changed in this patch.
    pub files: usize,
    /// Size of the patch file in bytes.
    pub size_bytes: u64,
}

/// Maps a file path to its content hash and the node IDs it contributes to the base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// SHA-256 content hash of the file.
    pub hash: String,
    /// Node IDs in the base that belong to this file.
    pub base_node_ids: Vec<String>,
}

/// Top-level manifest for the `.rpg/` directory.
///
/// Tracks the base file, patch history, compaction thresholds, and
/// a file index mapping paths to their node IDs in the base graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Storage format version.
    pub version: u32,
    /// Repository name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_name: Option<String>,
    /// Repository description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_info: Option<String>,
    /// Base file metadata.
    pub base: BaseInfo,
    /// Ordered list of patch records.
    #[serde(default)]
    pub patches: Vec<PatchInfo>,
    /// Compaction trigger thresholds.
    #[serde(default)]
    pub compaction_threshold: CompactionThreshold,
    /// Maps file paths to their hashes and node IDs in the base.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub file_index: HashMap<String, FileEntry>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            version: 1,
            repo_name: None,
            repo_info: None,
            base: BaseInfo {
                timestamp: 0,
                node_count: 0,
                edge_count: 0,
                file_hash: None,
                compressed: false,
            },
            patches: Vec::new(),
            compaction_threshold: CompactionThreshold::default(),
            file_index: HashMap::new(),
        }
    }
}

impl Manifest {
    #[must_use]
    pub fn new(repo_name: impl Into<String>) -> Self {
        Self {
            repo_name: Some(repo_name.into()),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn next_patch_seq(&self) -> u32 {
        self.patches.last().map_or(1, |p| p.seq + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_serialize_roundtrip() {
        let mut manifest = Manifest::new("test-repo");
        manifest.base = BaseInfo {
            timestamp: 1713014400,
            node_count: 100,
            edge_count: 200,
            file_hash: Some("sha256:abc".to_string()),
            compressed: false,
        };
        manifest.patches.push(PatchInfo {
            seq: 1,
            timestamp: 1713015000,
            files: 3,
            size_bytes: 12340,
        });
        manifest.file_index.insert(
            "src/main.rs".to_string(),
            FileEntry {
                hash: "sha256:def".to_string(),
                base_node_ids: vec!["node_0".to_string()],
            },
        );

        let json = serde_json::to_string(&manifest).unwrap();
        let loaded: Manifest = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.repo_name.as_deref(), Some("test-repo"));
        assert_eq!(loaded.base.node_count, 100);
        assert_eq!(loaded.patches.len(), 1);
        assert_eq!(loaded.patches[0].seq, 1);
        assert_eq!(loaded.file_index.len(), 1);
    }

    #[test]
    fn test_next_patch_seq_empty() {
        let manifest = Manifest::default();
        assert_eq!(manifest.next_patch_seq(), 1);
    }

    #[test]
    fn test_next_patch_seq_with_patches() {
        let mut manifest = Manifest::default();
        manifest.patches.push(PatchInfo {
            seq: 1,
            timestamp: 100,
            files: 1,
            size_bytes: 100,
        });
        manifest.patches.push(PatchInfo {
            seq: 2,
            timestamp: 200,
            files: 1,
            size_bytes: 100,
        });
        assert_eq!(manifest.next_patch_seq(), 3);
    }

    #[test]
    fn test_compaction_threshold_default() {
        let threshold = CompactionThreshold::default();
        assert_eq!(threshold.max_patches, 10);
        assert!((threshold.max_size_ratio - 0.5).abs() < f64::EPSILON);
    }
}
