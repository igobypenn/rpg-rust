use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionThreshold {
    pub max_patches: usize,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseInfo {
    pub timestamp: u64,
    pub node_count: usize,
    pub edge_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_hash: Option<String>,
    #[serde(default)]
    pub compressed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchInfo {
    pub seq: u32,
    pub timestamp: u64,
    pub files: usize,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub hash: String,
    pub base_node_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_info: Option<String>,
    pub base: BaseInfo,
    #[serde(default)]
    pub patches: Vec<PatchInfo>,
    #[serde(default)]
    pub compaction_threshold: CompactionThreshold,
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
    pub fn new(repo_name: String) -> Self {
        Self {
            repo_name: Some(repo_name),
            ..Default::default()
        }
    }

    pub fn next_patch_seq(&self) -> u32 {
        self.patches.last().map(|p| p.seq + 1).unwrap_or(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_serialize_roundtrip() {
        let mut manifest = Manifest::new("test-repo".to_string());
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
