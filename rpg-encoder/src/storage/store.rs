use std::path::{Path, PathBuf};

use crate::core::{Node, NodeId};
use crate::error::{Result, RpgError};
use crate::incremental::{compute_file_hash, RpgSnapshot};
use crate::storage::base::{node_id_from_str, parse_category, BaseSnapshot};
use crate::storage::manifest::{BaseInfo, FileEntry, Manifest, PatchInfo};
use crate::storage::patch::Patch;

const RPG_DIR: &str = ".rpg";
const MANIFEST_FILE: &str = "manifest.json";
const BASE_FILE: &str = "base.json";
const PATCHES_DIR: &str = "patches";
const STORE_VERSION: u32 = 1;

/// Storage backend for persisting RPG graphs as a base JSON file with
/// append-only global patch files.
///
/// # Directory Layout
///
/// ```text
/// .rpg/
/// ├── manifest.json
/// ├── base.json
/// └── patches/
///     ├── 001.json
///     └── 002.json
/// ```
pub struct RpgStore {
    root: PathBuf,
    manifest: Manifest,
}

impl RpgStore {
    /// Initialize a new `.rpg/` directory at the repository root.
    ///
    /// # Errors
    ///
    /// Returns an error if directory creation or manifest write fails.
    pub fn init(repo_path: &Path) -> Result<Self> {
        let rpg_dir = repo_path.join(RPG_DIR);
        std::fs::create_dir_all(rpg_dir.join(PATCHES_DIR))?;

        let repo_name = repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repository")
            .to_string();

        let manifest = Manifest::new(repo_name);
        let store = Self {
            root: rpg_dir,
            manifest,
        };

        store.write_manifest()?;

        Ok(store)
    }

    /// Open an existing `.rpg/` directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the `.rpg/` directory or manifest file does not exist,
    /// or if the format version is unsupported.
    pub fn open(repo_path: &Path) -> Result<Self> {
        let rpg_dir = repo_path.join(RPG_DIR);
        let manifest_path = rpg_dir.join(MANIFEST_FILE);

        if !manifest_path.exists() {
            return Err(RpgError::Incremental(format!(
                "No .rpg directory found at {}",
                rpg_dir.display()
            )));
        }

        let manifest_json = std::fs::read_to_string(&manifest_path)?;
        let manifest: Manifest = serde_json::from_str(&manifest_json)?;

        if manifest.version != STORE_VERSION {
            return Err(RpgError::Incremental(format!(
                "Unsupported .rpg version: {} (expected {})",
                manifest.version, STORE_VERSION
            )));
        }

        Ok(Self {
            root: rpg_dir,
            manifest,
        })
    }

    /// Load the full RPG: base + all patches applied.
    ///
    /// # Errors
    ///
    /// Returns an error if the base file is missing or corrupt,
    /// or if any patch file cannot be read or parsed.
    pub fn load(&self) -> Result<RpgSnapshot> {
        let (_base_path, base_bytes) = if self.manifest.base.compressed {
            let zst_path = self.root.join("base.json.zst");
            if !zst_path.exists() {
                return Err(RpgError::Incremental(
                    "Compressed base file not found".to_string(),
                ));
            }
            let compressed = std::fs::read(&zst_path)?;
            let decompressed = decompress_if_needed(&compressed)?;
            (zst_path, decompressed)
        } else {
            let path = self.root.join(BASE_FILE);
            if !path.exists() {
                return Err(RpgError::Incremental("No base file found".to_string()));
            }
            let bytes = std::fs::read(&path)?;
            (path, bytes)
        };

        let base_json = String::from_utf8(base_bytes)
            .map_err(|e| RpgError::Incremental(format!("Base is not valid UTF-8: {}", e)))?;
        let base: BaseSnapshot = serde_json::from_str(&base_json)
            .map_err(|e| RpgError::Incremental(format!("Failed to parse base: {}", e)))?;

        let repo_dir = self.root.parent().unwrap_or_else(|| Path::new("."));
        let repo_name = self.manifest.repo_name.as_deref().unwrap_or("repository");
        let mut snapshot = base.into_snapshot(repo_dir, repo_name);

        for patch_info in &self.manifest.patches {
            let patch_path = self
                .root
                .join(PATCHES_DIR)
                .join(format!("{:03}.json", patch_info.seq));
            if !patch_path.exists() {
                tracing::warn!("Patch file missing: {}", patch_path.display());
                break;
            }

            let patch_json = std::fs::read_to_string(&patch_path)?;
            let patch: Patch = serde_json::from_str(&patch_json).map_err(|e| {
                RpgError::Incremental(format!("Failed to parse patch {}: {}", patch_info.seq, e))
            })?;

            apply_patch(&mut snapshot, &patch);
        }

        snapshot.rebuild_node_id_index();
        snapshot.build_reverse_deps();

        Ok(snapshot)
    }

    /// Save a snapshot as the base (overwrites existing base and clears patches).
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or file write fails.
    pub fn save_base(&mut self, snapshot: &RpgSnapshot) -> Result<()> {
        self.manifest.repo_name = Some(snapshot.repo_name.clone());

        let base = BaseSnapshot::from_snapshot(snapshot);
        let base_json = serde_json::to_string(&base)
            .map_err(|e| RpgError::Incremental(format!("Failed to serialize base: {}", e)))?;

        let base_path = self.root.join(BASE_FILE);
        atomic_write(&base_path, base_json.as_bytes())?;

        let base_hash = compute_file_hash(&base_path)?;
        self.manifest.base = BaseInfo {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            node_count: snapshot.graph.node_count(),
            edge_count: snapshot.graph.edge_count(),
            file_hash: Some(base_hash),
            compressed: false,
        };

        self.manifest.patches.clear();
        self.rebuild_file_index(snapshot);

        self.write_manifest()?;

        Ok(())
    }

    /// Write a new patch file and update the manifest.
    ///
    /// # Errors
    ///
    /// Returns an error if patch serialization or file write fails.
    pub fn write_patch(&mut self, patch: &Patch) -> Result<()> {
        let patch_json = serde_json::to_string(patch)
            .map_err(|e| RpgError::Incremental(format!("Failed to serialize patch: {}", e)))?;

        let patch_path = self
            .root
            .join(PATCHES_DIR)
            .join(format!("{:03}.json", patch.seq));
        atomic_write(&patch_path, patch_json.as_bytes())?;

        let patch_size = patch_json.len() as u64;
        let files_changed = patch.changes.added_files.len()
            + patch.changes.deleted_files.len()
            + patch.changes.modified_files.len();

        self.manifest.patches.push(PatchInfo {
            seq: patch.seq,
            timestamp: patch.timestamp,
            files: files_changed,
            size_bytes: patch_size,
        });

        self.write_manifest()?;

        Ok(())
    }

    /// Check whether compaction thresholds are exceeded.
    #[must_use]
    pub fn should_compact(&self) -> bool {
        let patch_count = self.manifest.patches.len();
        if patch_count == 0 {
            return false;
        }
        if patch_count >= self.manifest.compaction_threshold.max_patches {
            return true;
        }
        let total_patch_size: u64 = self.manifest.patches.iter().map(|p| p.size_bytes).sum();
        let base_path = self.root.join(BASE_FILE);
        let base_size = std::fs::metadata(&base_path).map(|m| m.len()).unwrap_or(0);
        if base_size > 0 {
            let threshold_times_100 =
                (self.manifest.compaction_threshold.max_size_ratio * 100.0) as u64;
            if total_patch_size * 100 >= base_size * threshold_times_100 {
                return true;
            }
        }
        false
    }

    /// Compact: merge all patches into a new base file.
    ///
    /// # Errors
    ///
    /// Returns an error if loading, saving, or file operations fail.
    pub fn compact(&mut self) -> Result<()> {
        let snapshot = self.load()?;

        let old_base = self.root.join(BASE_FILE);
        let backup = self.root.join("base.json.bak");
        if old_base.exists() {
            std::fs::copy(&old_base, &backup)?;
        }

        self.save_base(&snapshot)?;

        for patch_info in &self.manifest.patches {
            let patch_path = self
                .root
                .join(PATCHES_DIR)
                .join(format!("{:03}.json", patch_info.seq));
            if patch_path.exists() {
                if let Err(e) = std::fs::remove_file(&patch_path) {
                    tracing::warn!(
                        "Failed to remove patch file {}: {}",
                        patch_path.display(),
                        e
                    );
                }
            }
        }

        if let Err(e) = std::fs::remove_file(&backup) {
            tracing::warn!("Failed to remove backup file {}: {}", backup.display(), e);
        }

        Ok(())
    }

    /// Number of unapplied patches.
    #[must_use]
    pub fn patch_count(&self) -> usize {
        self.manifest.patches.len()
    }

    /// Read-only access to the manifest.
    #[must_use]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn write_manifest(&self) -> Result<()> {
        let manifest_json = serde_json::to_string_pretty(&self.manifest)
            .map_err(|e| RpgError::Incremental(format!("Failed to serialize manifest: {}", e)))?;

        let manifest_path = self.root.join(MANIFEST_FILE);
        atomic_write(&manifest_path, manifest_json.as_bytes())?;

        Ok(())
    }

    fn rebuild_file_index(&mut self, snapshot: &RpgSnapshot) {
        self.manifest.file_index.clear();

        for node in snapshot.graph.nodes() {
            if let Some(ref path) = node.path {
                let path_str = path.to_string_lossy().to_string();
                let entry = self
                    .manifest
                    .file_index
                    .entry(path_str)
                    .or_insert_with(|| FileEntry {
                        hash: String::new(),
                        base_node_ids: Vec::new(),
                    });
                entry
                    .base_node_ids
                    .push(format!("node_{}", node.id.index()));
            }
        }

        for (path, hash) in &snapshot.file_hashes {
            let path_str = path.to_string_lossy().to_string();
            if let Some(entry) = self.manifest.file_index.get_mut(&path_str) {
                entry.hash = hash.clone();
            }
        }
    }
}

fn apply_patch(snapshot: &mut RpgSnapshot, patch: &Patch) {
    for path in &patch.changes.deleted_files {
        snapshot.graph.remove_file_nodes(path);
        snapshot.file_hashes.remove(path);
        snapshot.unit_cache.remove(path);
    }

    for (path_str, file_patch) in &patch.changes.modified_files {
        for node_id_str in &file_patch.removed_node_ids {
            if let Some(idx) = node_id_from_str(node_id_str) {
                snapshot.graph.remove_node(idx);
            }
        }

        for serialized_node in &file_patch.added_nodes {
            let category = parse_category(&serialized_node.category);
            let mut node = Node::new(
                NodeId::new(0),
                category,
                &serialized_node.kind,
                &serialized_node.language,
                &serialized_node.name,
            );
            if let Some(ref path) = serialized_node.path {
                node = node.with_path(PathBuf::from(path));
            }
            snapshot.graph.add_node(node);
        }

        let file_path = PathBuf::from(path_str);
        snapshot
            .file_hashes
            .insert(file_path.clone(), file_patch.new_hash.clone());

        if !file_patch.added_nodes.is_empty() || !file_patch.removed_node_ids.is_empty() {
            snapshot.unit_cache.remove(&file_path);
        }
    }
}

fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn decompress_if_needed(data: &[u8]) -> Result<Vec<u8>> {
    #[cfg(feature = "compression")]
    {
        zstd::decode_all(data)
            .map_err(|e| RpgError::Incremental(format!("zstd decompression failed: {}", e)))
    }
    #[cfg(not(feature = "compression"))]
    {
        let _ = data;
        Err(RpgError::Incremental(
            "Base is compressed but 'compression' feature is not enabled".to_string(),
        ))
    }
}

#[cfg(feature = "compression")]
impl RpgStore {
    /// Compress the base file with zstd.
    ///
    /// Replaces `base.json` with `base.json.zst`.
    ///
    /// # Errors
    ///
    /// Returns an error if no base file exists or compression fails.
    pub fn compress_base(&mut self) -> Result<()> {
        let base_path = self.root.join(BASE_FILE);
        if !base_path.exists() {
            return Err(RpgError::Incremental(
                "No base file to compress".to_string(),
            ));
        }

        let base_bytes = std::fs::read(&base_path)?;
        let compressed = zstd::encode_all(&base_bytes[..], 3)
            .map_err(|e| RpgError::Incremental(format!("zstd compression failed: {}", e)))?;

        let zst_path = self.root.join("base.json.zst");
        atomic_write(&zst_path, &compressed)?;

        let backup = self.root.join("base.json.uncompressed");
        std::fs::rename(&base_path, &backup)?;

        self.manifest.base.compressed = true;
        self.write_manifest()?;

        if let Err(e) = std::fs::remove_file(&backup) {
            tracing::warn!("Failed to remove uncompressed backup: {}", e);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::EdgeType;
    use crate::core::NodeCategory;
    use crate::encoder::SerializedNode;
    use crate::storage::patch::{FilePatch, Patch};
    use std::collections::HashMap;

    #[test]
    fn test_init_creates_directory() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();

        let store = RpgStore::init(repo_path).unwrap();

        assert!(repo_path.join(RPG_DIR).exists());
        assert!(repo_path.join(RPG_DIR).join(MANIFEST_FILE).exists());
        assert!(repo_path.join(RPG_DIR).join(PATCHES_DIR).exists());
        assert_eq!(store.manifest.version, STORE_VERSION);
    }

    #[test]
    fn test_save_base_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();

        let mut store = RpgStore::init(repo_path).unwrap();

        let mut snapshot = RpgSnapshot::new("test-repo", repo_path);
        let n1 = snapshot.graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Repository,
            "repository",
            "",
            "test-repo",
        ));
        let n2 = snapshot.graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::File,
                "file",
                "rust",
                "main.rs",
            )
            .with_path(PathBuf::from("src/main.rs")),
        );
        let n3 = snapshot.graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "function",
            "rust",
            "main",
        ));
        snapshot.graph.add_typed_edge(n1, n2, EdgeType::Contains);
        snapshot.graph.add_typed_edge(n2, n3, EdgeType::Contains);

        store.save_base(&snapshot).unwrap();

        let store2 = RpgStore::open(repo_path).unwrap();
        let loaded = store2.load().unwrap();

        assert_eq!(loaded.graph.node_count(), 3);
        assert_eq!(loaded.graph.edge_count(), 2);
        assert_eq!(loaded.repo_name, "test-repo");
    }

    #[test]
    fn test_open_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let result = RpgStore::open(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_write_patch_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();

        let mut store = RpgStore::init(repo_path).unwrap();

        let mut snapshot = RpgSnapshot::new("test", repo_path);
        let _n = snapshot.graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "function",
            "rust",
            "original",
        ));
        store.save_base(&snapshot).unwrap();

        let patch = Patch::new(1, 0);
        let file_patch = FilePatch {
            old_hash: "sha256:old".to_string(),
            new_hash: "sha256:new".to_string(),
            removed_node_ids: vec!["node_0".to_string()],
            added_nodes: vec![SerializedNode {
                id: "node_1".to_string(),
                category: "function".to_string(),
                kind: "function".to_string(),
                language: "rust".to_string(),
                name: "updated".to_string(),
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
            }],
            removed_edges: Vec::new(),
            added_edges: Vec::new(),
        };
        let mut patch = patch;
        patch
            .changes
            .modified_files
            .insert("src/main.rs".to_string(), file_patch);
        patch.stats.nodes_added = 1;
        patch.stats.nodes_removed = 1;

        store.write_patch(&patch).unwrap();
        assert_eq!(store.patch_count(), 1);

        let store2 = RpgStore::open(repo_path).unwrap();
        let loaded = store2.load().unwrap();

        assert_eq!(loaded.graph.node_count(), 1);
        let nodes: Vec<_> = loaded.graph.nodes().collect();
        assert_eq!(nodes[0].name, "updated");
    }

    #[test]
    fn test_should_compact_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = RpgStore::init(dir.path()).unwrap();
        assert!(!store.should_compact());
    }

    #[test]
    fn test_compact_clears_patches() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();

        let mut store = RpgStore::init(repo_path).unwrap();

        let mut snapshot = RpgSnapshot::new("test", repo_path);
        snapshot.graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "function",
            "rust",
            "foo",
        ));
        store.save_base(&snapshot).unwrap();

        let patch = Patch::new(1, 0);
        store.write_patch(&patch).unwrap();
        assert_eq!(store.patch_count(), 1);

        store.compact().unwrap();
        assert_eq!(store.patch_count(), 0);

        let store2 = RpgStore::open(repo_path).unwrap();
        let loaded = store2.load().unwrap();
        assert_eq!(loaded.graph.node_count(), 1);
    }

    #[test]
    fn test_manifest_file_index_after_save() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();

        let mut store = RpgStore::init(repo_path).unwrap();

        let mut snapshot = RpgSnapshot::new("test", repo_path);
        snapshot.graph.add_node(
            Node::new(NodeId::new(0), NodeCategory::File, "file", "rust", "lib.rs")
                .with_path(PathBuf::from("src/lib.rs")),
        );
        snapshot.graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::Function,
                "function",
                "rust",
                "helper",
            )
            .with_path(PathBuf::from("src/lib.rs")),
        );

        store.save_base(&snapshot).unwrap();

        let store2 = RpgStore::open(repo_path).unwrap();
        assert!(store2.manifest().file_index.contains_key("src/lib.rs"));
    }

    #[cfg(feature = "compression")]
    #[test]
    fn test_compress_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();

        let mut store = RpgStore::init(repo_path).unwrap();

        let mut snapshot = RpgSnapshot::new("test", repo_path);
        snapshot.graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "function",
            "rust",
            "foo",
        ));
        store.save_base(&snapshot).unwrap();

        store.compress_base().unwrap();
        assert!(store.manifest().base.compressed);
        assert!(!repo_path.join(".rpg/base.json").exists());
        assert!(repo_path.join(".rpg/base.json.zst").exists());

        let loaded = store.load().unwrap();
        assert_eq!(loaded.graph.node_count(), 1);
    }
}
