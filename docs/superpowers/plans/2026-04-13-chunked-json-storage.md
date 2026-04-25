# RPG Chunked JSON Storage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- []`) syntax for tracking.

**Goal:** Add a storage backend that persists RPG graphs as a JSON base file + append-only global patch files, with automatic compaction.

**Architecture:** A new `storage` module under `rpg-encoder/src/storage/` provides `RpgStore` — the single entry point for init, load, save, patch, and compact. Patch files record per-file node/edge diffs using the existing `SerializedNode`/`SerializedEdge` types from `encoder/output.rs`. The `RpgSnapshot` is extended with serialization helpers for the base format. The `RpgEncoder` gains a `store()` method.

**Tech Stack:** serde, serde_json (existing), sha2 (existing), chrono (existing), std::fs

**Spec:** `docs/superpowers/specs/2026-04-13-chunked-json-storage-design.md`

---

### Task 1: Manifest types

**Files:**
- Create: `rpg-encoder/src/storage/manifest.rs`
- Create: `rpg-encoder/src/storage/mod.rs`
- Test: inline tests in `manifest.rs`

- [ ] **Step 1: Create `storage/mod.rs` with module declarations**

```rust
mod manifest;
mod patch;
mod store;

pub use manifest::{Manifest, BaseInfo, PatchInfo, FileEntry, CompactionThreshold};
pub use patch::{Patch, PatchChanges, FilePatch, RemovedEdge, PatchStats};
pub use store::RpgStore;
```

- [ ] **Step 2: Write failing tests for `Manifest` serialization**

In `rpg-encoder/src/storage/manifest.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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
        manifest.patches.push(PatchInfo { seq: 1, timestamp: 100, files: 1, size_bytes: 100 });
        manifest.patches.push(PatchInfo { seq: 2, timestamp: 200, files: 1, size_bytes: 100 });
        assert_eq!(manifest.next_patch_seq(), 3);
    }

    #[test]
    fn test_compaction_threshold_default() {
        let threshold = CompactionThreshold::default();
        assert_eq!(threshold.max_patches, 10);
        assert!((threshold.max_size_ratio - 0.5).abs() < f64::EPSILON);
    }
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p rpg-encoder manifest`
Expected: All 4 tests PASS

- [ ] **Step 4: Register `storage` module in `lib.rs`**

In `rpg-encoder/src/lib.rs`, add after the `incremental` module line:

```rust
pub mod storage;
```

And add to the public re-exports:

```rust
pub use storage::{Manifest, BaseInfo, PatchInfo, FileEntry, CompactionThreshold, RpgStore, Patch, PatchChanges, FilePatch, RemovedEdge, PatchStats};
```

- [ ] **Step 5: Run full test suite to verify no breakage**

Run: `cargo test -p rpg-encoder`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add rpg-encoder/src/storage/ rpg-encoder/src/lib.rs
git commit -m "feat(storage): add manifest types with serde serialization"
```

---

### Task 2: Patch types

**Files:**
- Create: `rpg-encoder/src/storage/patch.rs`
- Test: inline tests in `patch.rs`

- [ ] **Step 1: Write failing tests for `Patch` types**

In `rpg-encoder/src/storage/patch.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::encoder::output::{SerializedEdge, SerializedNode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedEdge {
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub edge_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePatch {
    pub old_hash: String,
    pub new_hash: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_node_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_nodes: Vec<SerializedNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_edges: Vec<RemovedEdge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_edges: Vec<SerializedEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchChanges {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_files: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deleted_files: Vec<PathBuf>,
    #[serde(default)]
    pub modified_files: HashMap<String, FilePatch>,
}

impl Default for PatchChanges {
    fn default() -> Self {
        Self {
            added_files: Vec::new(),
            deleted_files: Vec::new(),
            modified_files: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for PatchStats {
    fn default() -> Self {
        Self {
            files_added: 0,
            files_deleted: 0,
            files_modified: 0,
            nodes_added: 0,
            nodes_removed: 0,
            edges_added: 0,
            edges_removed: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    pub seq: u32,
    pub timestamp: u64,
    pub parent_seq: u32,
    pub changes: PatchChanges,
    pub stats: PatchStats,
}

impl Patch {
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
        }
    }

    #[test]
    fn test_patch_serialize_roundtrip() {
        let mut patch = Patch::new(1, 0);
        patch.changes.added_files.push(PathBuf::from("src/new.rs"));
        patch.changes.deleted_files.push(PathBuf::from("src/old.rs"));
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
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p rpg-encoder patch`
Expected: All 4 tests PASS

- [ ] **Step 3: Commit**

```bash
git add rpg-encoder/src/storage/patch.rs
git commit -m "feat(storage): add patch types with serde serialization"
```

---

### Task 3: Base serialization format

**Files:**
- Create: `rpg-encoder/src/storage/base.rs`
- Modify: `rpg-encoder/src/storage/mod.rs` — add `mod base;` and `pub use base::BaseSnapshot;`
- Test: inline tests in `base.rs`

The base file extends the existing `SerializedGraph` with `file_hashes` and `unit_cache` from `RpgSnapshot`. This task creates the type that `RpgStore` will serialize/deserialize.

- [ ] **Step 1: Write failing test for `BaseSnapshot` roundtrip**

In `rpg-encoder/src/storage/base.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::encoder::output::{SerializedEdge, SerializedGraph, SerializedNode};
use crate::incremental::{CachedUnit, RpgSnapshot, SNAPSHOT_VERSION};
use crate::core::{RpgGraph, Node, NodeCategory, NodeId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseSnapshot {
    #[serde(flatten)]
    pub graph: SerializedGraph,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub file_hashes: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub unit_cache: HashMap<String, Vec<CachedUnit>>,
    #[serde(default)]
    pub snapshot_version: u32,
}

impl BaseSnapshot {
    pub fn from_snapshot(snapshot: &RpgSnapshot) -> Self {
        let graph = crate::encoder::serialize_graph(&snapshot.graph);

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

    pub fn into_snapshot(self, repo_dir: &std::path::Path, repo_name: &str) -> RpgSnapshot {
        use crate::core::Edge;

        let mut graph = RpgGraph::new();

        for node_data in &self.graph.nodes {
            let category = parse_category(&node_data.category);
            let node = Node::new(NodeId::new(0), category, &node_data.kind, &node_data.language, &node_data.name);
            let node_id = graph.add_node(node);

            if let Some(ref path) = node_data.path {
                if let Some(node_mut) = graph.get_node_mut(node_id) {
                    node_mut.path = Some(PathBuf::from(path));
                }
            }
        }

        for edge_data in &self.graph.edges {
            let edge_type = parse_edge_type(&edge_data.edge_type);
            graph.add_edge(edge_data.source.clone(), edge_data.target.clone(), Edge::new(edge_type));
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
        snapshot.graph = graph;
        snapshot.file_hashes = file_hashes;
        snapshot.unit_cache = unit_cache;
        snapshot.encoding_timestamp = now;
        snapshot.last_modified = now;
        snapshot.rebuild_node_id_index();
        snapshot
    }
}

fn parse_category(s: &str) -> NodeCategory {
    match s {
        "repository" => NodeCategory::Repository,
        "directory" => NodeCategory::Directory,
        "file" => NodeCategory::File,
        "module" => NodeCategory::Module,
        "type" => NodeCategory::Type,
        "function" => NodeCategory::Function,
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

fn parse_edge_type(s: &str) -> crate::core::EdgeType {
    match s {
        "contains" => crate::core::EdgeType::Contains,
        "imports" => crate::core::EdgeType::Imports,
        "calls" => crate::core::EdgeType::Calls,
        "extends" => crate::core::EdgeType::Extends,
        "implements" => crate::core::EdgeType::Implements,
        "references" => crate::core::EdgeType::References,
        "depends_on" => crate::core::EdgeType::DependsOn,
        "ffi_binding" => crate::core::EdgeType::FfiBinding,
        "defines" => crate::core::EdgeType::Defines,
        "uses" => crate::core::EdgeType::Uses,
        "uses_type" => crate::core::EdgeType::UsesType,
        "implements_feature" => crate::core::EdgeType::ImplementsFeature,
        "belongs_to_feature" => crate::core::EdgeType::BelongsToFeature,
        "contains_feature" => crate::core::EdgeType::ContainsFeature,
        "belongs_to_component" => crate::core::EdgeType::BelongsToComponent,
        _ => crate::core::EdgeType::References,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_snapshot_serialize_roundtrip() {
        let mut snapshot = RpgSnapshot::new("test-repo", PathBuf::from("/tmp/test"));
        snapshot.graph.add_node(
            Node::new(NodeId::new(0), NodeCategory::File, "file", "rust", "main.rs")
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
        let snapshot = RpgSnapshot::new("test", PathBuf::from("/tmp"));
        let base = BaseSnapshot::from_snapshot(&snapshot);
        let json = serde_json::to_string(&base).unwrap();

        assert!(!json.contains("file_hashes"));
        assert!(!json.contains("unit_cache"));
    }

    #[test]
    fn test_parse_category() {
        assert!(matches!(parse_category("function"), NodeCategory::Function));
        assert!(matches!(parse_category("file"), NodeCategory::File));
        assert!(matches!(parse_category("type"), NodeCategory::Type));
        assert!(matches!(parse_category("functional_centroid"), NodeCategory::FunctionalCentroid));
    }

    #[test]
    fn test_parse_edge_type() {
        assert!(matches!(parse_edge_type("calls"), crate::core::EdgeType::Calls));
        assert!(matches!(parse_edge_type("contains"), crate::core::EdgeType::Contains));
        assert!(matches!(parse_edge_type("belongs_to_feature"), crate::core::EdgeType::BelongsToFeature));
    }
}
```

- [ ] **Step 2: Add `mod base` to `storage/mod.rs`**

Update `rpg-encoder/src/storage/mod.rs`:

```rust
mod base;
mod manifest;
mod patch;
mod store;

pub use base::BaseSnapshot;
pub use manifest::{Manifest, BaseInfo, PatchInfo, FileEntry, CompactionThreshold};
pub use patch::{Patch, PatchChanges, FilePatch, RemovedEdge, PatchStats};
pub use store::RpgStore;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p rpg-encoder base`
Expected: All 4 tests PASS

- [ ] **Step 4: Commit**

```bash
git add rpg-encoder/src/storage/
git commit -m "feat(storage): add BaseSnapshot type for base file serialization"
```

---

### Task 4: `RpgStore` — init, save_base, load

**Files:**
- Modify: `rpg-encoder/src/storage/store.rs` (create initially as stub, then implement)
- Test: inline tests in `store.rs`

- [ ] **Step 1: Write failing tests for init, save_base, and load**

In `rpg-encoder/src/storage/store.rs`:

```rust
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::core::{EdgeType, Node, NodeCategory, NodeId, RpgGraph};
use crate::error::{Result, RpgError};
use crate::incremental::{compute_file_hash, CachedUnit, RpgSnapshot, UnitType};
use crate::storage::base::BaseSnapshot;
use crate::storage::manifest::{BaseInfo, CompactionThreshold, FileEntry, Manifest, PatchInfo};
use crate::storage::patch::{FilePatch, Patch, PatchChanges, PatchStats};

const RPG_DIR: &str = ".rpg";
const MANIFEST_FILE: &str = "manifest.json";
const BASE_FILE: &str = "base.json";
const PATCHES_DIR: &str = "patches";
const STORE_VERSION: u32 = 1;

pub struct RpgStore {
    root: PathBuf,
    manifest: Manifest,
}

impl RpgStore {
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

    pub fn load(&self) -> Result<RpgSnapshot> {
        let base_path = self.root.join(BASE_FILE);

        if !base_path.exists() {
            return Err(RpgError::Incremental("No base file found".to_string()));
        }

        let base_json = std::fs::read_to_string(&base_path)?;
        let base: BaseSnapshot = serde_json::from_str(&base_json)
            .map_err(|e| RpgError::Incremental(format!("Failed to parse base: {}", e)))?;

        let repo_dir = self.root.parent().unwrap_or(Path::new("."));
        let repo_name = self.manifest.repo_name.as_deref().unwrap_or("repository");
        let mut snapshot = base.into_snapshot(repo_dir, repo_name);

        for patch_info in &self.manifest.patches {
            let patch_path = self.root.join(PATCHES_DIR).join(format!("{:03}.json", patch_info.seq));
            if !patch_path.exists() {
                tracing::warn!("Patch file missing: {}", patch_path.display());
                break;
            }

            let patch_json = std::fs::read_to_string(&patch_path)?;
            let patch: Patch = serde_json::from_str(&patch_json)
                .map_err(|e| RpgError::Incremental(format!("Failed to parse patch {}: {}", patch_info.seq, e)))?;

            apply_patch(&mut snapshot, &patch);
        }

        snapshot.rebuild_node_id_index();
        snapshot.build_reverse_deps();

        Ok(snapshot)
    }

    pub fn save_base(&mut self, snapshot: &RpgSnapshot) -> Result<()> {
        let repo_dir = snapshot.repo_dir.clone();
        let repo_name = snapshot.repo_name.clone();
        self.manifest.repo_name = Some(repo_name);

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

    pub fn write_patch(&mut self, patch: Patch) -> Result<()> {
        let patch_json = serde_json::to_string(&patch)
            .map_err(|e| RpgError::Incremental(format!("Failed to serialize patch: {}", e)))?;

        let patch_path = self.root.join(PATCHES_DIR).join(format!("{:03}.json", patch.seq));
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
            let ratio = total_patch_size as f64 / base_size as f64;
            if ratio >= self.manifest.compaction_threshold.max_size_ratio {
                return true;
            }
        }
        false
    }

    pub fn compact(&mut self) -> Result<()> {
        let snapshot = self.load()?;

        let old_base = self.root.join(BASE_FILE);
        let backup = self.root.join("base.json.bak");
        if old_base.exists() {
            std::fs::copy(&old_base, &backup)?;
        }

        self.save_base(&snapshot)?;

        for patch_info in &self.manifest.patches {
            let patch_path = self.root.join(PATCHES_DIR).join(format!("{:03}.json", patch_info.seq));
            if patch_path.exists() {
                let _ = std::fs::remove_file(&patch_path);
            }
        }

        let _ = std::fs::remove_file(&backup);

        Ok(())
    }

    pub fn patch_count(&self) -> usize {
        self.manifest.patches.len()
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn capture_patch(
        &self,
        before: &RpgSnapshot,
        after: &RpgSnapshot,
        file_diff: &crate::incremental::FileDiff,
    ) -> Patch {
        let seq = self.manifest.next_patch_seq();
        let parent_seq = self.manifest.patches.last().map(|p| p.seq).unwrap_or(0);
        let mut patch = Patch::new(seq, parent_seq);

        let before_node_ids: HashSet<String> = before
            .graph
            .nodes()
            .map(|n| format!("node_{}", n.id.index()))
            .collect();
        let after_node_ids: HashSet<String> = after
            .graph
            .nodes()
            .map(|n| format!("node_{}", n.id.index()))
            .collect();

        let before_edges: HashSet<(String, String, String)> = before
            .graph
            .edges()
            .map(|(s, t, e)| {
                (
                    format!("node_{}", s.index()),
                    format!("node_{}", t.index()),
                    format!("{:?}", e.edge_type).to_lowercase(),
                )
            })
            .collect();
        let after_edges: HashSet<(String, String, String)> = after
            .graph
            .edges()
            .map(|(s, t, e)| {
                (
                    format!("node_{}", s.index()),
                    format!("node_{}", t.index()),
                    format!("{:?}", e.edge_type).to_lowercase(),
                )
            })
            .collect();

        let removed_node_ids: Vec<String> = before_node_ids.difference(&after_node_ids).cloned().collect();
        let added_node_ids: Vec<String> = after_node_ids.difference(&before_node_ids).cloned().collect();

        let added_nodes: Vec<crate::encoder::output::SerializedNode> = after
            .graph
            .nodes()
            .filter(|n| added_node_ids.contains(&format!("node_{}", n.id.index())))
            .map(|n| crate::encoder::output::SerializedNode {
                id: format!("node_{}", n.id.index()),
                category: format!("{:?}", n.category).to_lowercase(),
                kind: n.kind.clone(),
                language: n.language.clone(),
                name: n.name.clone(),
                path: n.path.as_ref().map(|p| p.to_string_lossy().to_string()),
                location: n.location.as_ref().map(|l| l.into()),
                metadata: n.metadata.clone(),
            })
            .collect();

        let removed_edges: Vec<crate::storage::patch::RemovedEdge> = before_edges
            .difference(&after_edges)
            .map(|(s, t, et)| crate::storage::patch::RemovedEdge {
                source: s.clone(),
                target: t.clone(),
                edge_type: et.clone(),
            })
            .collect();

        let added_edges: Vec<crate::encoder::output::SerializedEdge> = after_edges
            .difference(&before_edges)
            .filter_map(|(s, t, et)| {
                after.graph.edges().find_map(|(es, et2, e)| {
                    let es_str = format!("node_{}", es.index());
                    let et_str = format!("node_{}", et2.index());
                    if es_str == *s && et_str == *t && format!("{:?}", e.edge_type).to_lowercase() == *et {
                        Some(crate::encoder::output::SerializedEdge {
                            source: s.clone(),
                            target: t.clone(),
                            edge_type: et.clone(),
                            metadata: e.metadata.clone(),
                        })
                    } else {
                        None
                    }
                })
            })
            .collect();

        for path in &file_diff.added {
            patch.changes.added_files.push(path.clone());
        }
        for path in &file_diff.deleted {
            patch.changes.deleted_files.push(path.clone());
        }
        for modified in &file_diff.modified {
            let path_str = modified.path.to_string_lossy().to_string();

            let file_removed: Vec<String> = removed_node_ids
                .iter()
                .filter(|id| {
                    after.graph.nodes().any(|n| {
                        let nid = format!("node_{}", n.id.index());
                        nid == *id
                    })
                })
                .cloned()
                .collect();

            let file_added: Vec<crate::encoder::output::SerializedNode> = added_nodes
                .iter()
                .filter(|n| n.path.as_deref() == Some(path_str.as_str()))
                .cloned()
                .collect();

            patch.changes.modified_files.insert(
                path_str,
                FilePatch {
                    old_hash: modified.old_hash.clone(),
                    new_hash: modified.new_hash.clone(),
                    removed_node_ids: file_removed,
                    added_nodes: file_added,
                    removed_edges: Vec::new(),
                    added_edges: Vec::new(),
                },
            );
        }

        patch.stats.files_added = file_diff.added.len();
        patch.stats.files_deleted = file_diff.deleted.len();
        patch.stats.files_modified = file_diff.modified.len();
        patch.stats.nodes_added = added_nodes.len();
        patch.stats.nodes_removed = removed_node_ids.len();
        patch.stats.edges_added = added_edges.len();
        patch.stats.edges_removed = removed_edges.len();

        patch
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
                let entry = self.manifest.file_index.entry(path_str).or_insert_with(|| {
                    FileEntry {
                        hash: String::new(),
                        base_node_ids: Vec::new(),
                    }
                });
                entry.base_node_ids.push(format!("node_{}", node.id.index()));
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

    for path in &patch.changes.added_files {
        if let Some(hash) = patch.changes.modified_files.get(&path.to_string_lossy().to_string()) {
            snapshot.file_hashes.insert(path.clone(), hash.new_hash.clone());
        }
    }

    for (path_str, file_patch) in &patch.changes.modified_files {
        for node_id_str in &file_patch.removed_node_ids {
            if let Some(idx) = node_id_from_str(node_id_str) {
                snapshot.graph.remove_node(idx);
            }
        }

        for serialized_node in &file_patch.added_nodes {
            let category = crate::storage::base::parse_category(&serialized_node.category);
            let node = Node::new(
                NodeId::new(0),
                category,
                &serialized_node.kind,
                &serialized_node.language,
                &serialized_node.name,
            );
            let new_id = snapshot.graph.add_node(node);

            if let Some(ref path) = serialized_node.path {
                if let Some(node_mut) = snapshot.graph.get_node_mut(new_id) {
                    node_mut.path = Some(PathBuf::from(path));
                }
            }
        }

        let path = PathBuf::from(path_str);
        snapshot.file_hashes.insert(path.clone(), file_patch.new_hash.clone());

        if !file_patch.added_nodes.is_empty() || !file_patch.removed_node_ids.is_empty() {
            snapshot.unit_cache.remove(&path);
        }
    }
}

fn node_id_from_str(s: &str) -> Option<NodeId> {
    let s = s.strip_prefix("node_")?;
    let idx: usize = s.parse().ok()?;
    Some(NodeId::new(idx))
}

fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::EdgeType;

    #[test]
    fn test_init_creates_directory() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();

        let store = RpgStore::init(repo_path).unwrap();

        assert!(repo_path.join(RPG_DIR).exists());
        assert!(repo_path.join(RPG_DIR).join(MANIFEST_FILE).exists());
        assert!(repo_path.join(RPG_DIR).join(PATCHES_DIR).exists());
        assert_eq!(store.manifest.version, STORE_VERSION);
        assert_eq!(store.manifest.repo_name.as_deref(), Some("temp"));
    }

    #[test]
    fn test_save_base_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();

        let mut store = RpgStore::init(repo_path).unwrap();

        let mut snapshot = RpgSnapshot::new("test-repo", repo_path);
        let n1 = snapshot.graph.add_node(
            Node::new(NodeId::new(0), NodeCategory::Repository, "repository", "", "test-repo"),
        );
        let n2 = snapshot.graph.add_node(
            Node::new(NodeId::new(0), NodeCategory::File, "file", "rust", "main.rs")
                .with_path(PathBuf::from("src/main.rs")),
        );
        let n3 = snapshot.graph.add_node(
            Node::new(NodeId::new(0), NodeCategory::Function, "function", "rust", "main"),
        );
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
        let _n = snapshot.graph.add_node(
            Node::new(NodeId::new(0), NodeCategory::Function, "function", "rust", "original"),
        );
        store.save_base(&snapshot).unwrap();

        let mut patch = Patch::new(1, 0);
        patch.changes.modified_files.insert(
            "src/main.rs".to_string(),
            FilePatch {
                old_hash: "sha256:old".to_string(),
                new_hash: "sha256:new".to_string(),
                removed_node_ids: vec![format!("node_0")],
                added_nodes: vec![crate::encoder::output::SerializedNode {
                    id: "node_1".to_string(),
                    category: "function".to_string(),
                    kind: "function".to_string(),
                    language: "rust".to_string(),
                    name: "updated".to_string(),
                    path: Some("src/main.rs".to_string()),
                    location: None,
                    metadata: HashMap::new(),
                }],
                removed_edges: Vec::new(),
                added_edges: Vec::new(),
            },
        );
        patch.stats.nodes_added = 1;
        patch.stats.nodes_removed = 1;

        store.write_patch(patch).unwrap();
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
        snapshot.graph.add_node(
            Node::new(NodeId::new(0), NodeCategory::Function, "function", "rust", "foo"),
        );
        store.save_base(&snapshot).unwrap();

        let patch = Patch::new(1, 0);
        store.write_patch(patch).unwrap();
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
            Node::new(NodeId::new(0), NodeCategory::Function, "function", "rust", "helper")
                .with_path(PathBuf::from("src/lib.rs")),
        );

        store.save_base(&snapshot).unwrap();

        let store2 = RpgStore::open(repo_path).unwrap();
        assert!(store2.manifest().file_index.contains_key("src/lib.rs"));
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p rpg-encoder store`
Expected: All 7 tests PASS

- [ ] **Step 3: Commit**

```bash
git add rpg-encoder/src/storage/store.rs
git commit -m "feat(storage): implement RpgStore with init, save_base, load, write_patch, compact"
```

---

### Task 5: Integration — wire `RpgStore` into `RpgEncoder`

**Files:**
- Modify: `rpg-encoder/src/encoder/mod.rs` — add `store()` method to `RpgEncoder`
- Modify: `rpg-encoder/src/lib.rs` — export new types
- Test: integration test in `rpg-encoder/tests/storage/integration_test.rs`

- [ ] **Step 1: Add `store` field to `RpgEncoder`**

In `rpg-encoder/src/encoder/mod.rs`, add a `store` field and accessor:

```rust
use std::path::{Path, PathBuf};
use crate::storage::RpgStore;

pub struct RpgEncoder {
    registry: ParserRegistry,
    root: Option<PathBuf>,
    graph: Option<RpgGraph>,
    store: Option<RpgStore>,
}
```

Update `Self::new()` to initialize `store: None`.

Add methods:

```rust
/// Get the RPG store (if initialized).
pub fn store(&self) -> Option<&RpgStore> {
    self.store.as_ref()
}

/// Get a mutable reference to the RPG store.
pub fn store_mut(&mut self) -> Option<&mut RpgStore> {
    self.store.as_mut()
}

/// Initialize the RPG store for the given repo path.
/// Creates `.rpg/` directory and manifest. Returns &mut RpgStore.
pub fn init_store(&mut self, repo_path: &Path) -> Result<&RpgStore> {
    let store = RpgStore::init(repo_path)?;
    self.store = Some(store);
    Ok(self.store.as_ref().unwrap())
}

/// Open an existing RPG store.
pub fn open_store(&mut self, repo_path: &Path) -> Result<&RpgStore> {
    let store = RpgStore::open(repo_path)?;
    self.store = Some(store);
    Ok(self.store.as_ref().unwrap())
}
```

Update the `Default` impl:

```rust
impl Default for RpgEncoder {
    fn default() -> Self {
        Self {
            registry: Self::default_registry().expect("Failed to initialize RpgEncoder"),
            root: None,
            graph: None,
            store: None,
        }
    }
}
```

Extract registry creation to a private method so `Default` and `new()` share it:

```rust
fn default_registry() -> Result<ParserRegistry> {
    let mut registry = ParserRegistry::new();
    let parser = crate::languages::RustParser::new()
        .map_err(|e| RpgError::parser_init("rust", e.to_string()))?;
    registry.register(Box::new(parser));
    register_parsers!(
        registry,
        crate::languages::PythonParser,
        // ... all parsers
    );
    Ok(registry)
}
```

- [ ] **Step 2: Write integration test**

In `rpg-encoder/tests/storage/integration_test.rs`:

```rust
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use rpg_encoder::{RpgEncoder, RpgSnapshot, Node, NodeCategory, NodeId, EdgeType, compute_hash};

fn create_test_repo(dir: &Path) {
    let src = dir.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("main.rs"), "fn main() {}\n").unwrap();
}

#[test]
fn test_full_encode_store_load_cycle() {
    let dir = TempDir::new().unwrap();
    create_test_repo(dir.path());
    let repo = dir.path().join("src");

    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(&repo).unwrap();
    assert!(result.graph.node_count() > 0);

    let store = encoder.init_store(dir.path()).unwrap();
    assert_eq!(store.patch_count(), 0);

    let mut snapshot = RpgSnapshot::new("test", dir.path());
    snapshot.graph = result.graph.clone();
    snapshot.compute_file_hashes().ok();

    encoder.store_mut().unwrap().save_base(&snapshot).unwrap();

    let loaded = RpgStore::open(dir.path()).unwrap().load().unwrap();
    assert_eq!(loaded.graph.node_count(), snapshot.graph.node_count());
}

#[test]
fn test_open_store_from_existing() {
    let dir = TempDir::new().unwrap();
    create_test_repo(dir.path());

    let mut encoder = RpgEncoder::new().unwrap();
    encoder.init_store(dir.path()).unwrap();

    let encoder2 = RpgEncoder::new().unwrap();
    let store = encoder2.open_store(dir.path()).unwrap();
    assert_eq!(store.patch_count(), 0);
}
```

- [ ] **Step 3: Run integration test**

Run: `cargo test -p rpg-encoder --test integration_test`
Expected: PASS

- [ ] **Step 4: Run full test suite**

Run: `cargo test -p rpg-encoder`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add rpg-encoder/src/encoder/mod.rs rpg-encoder/src/lib.rs rpg-encoder/tests/
git commit -m "feat(storage): wire RpgStore into RpgEncoder with init/open/store methods"
```

---

### Task 6: Optional zstd compression

**Files:**
- Modify: `rpg-encoder/Cargo.toml` — add optional `zstd` dependency
- Modify: `rpg-encoder/src/storage/store.rs` — add `compress_base()` and handle compressed load
- Test: inline tests in `store.rs`

- [ ] **Step 1: Add optional zstd dependency**

In `rpg-encoder/Cargo.toml`, under `[dependencies]`:

```toml
zstd = { version = "0.13", optional = true }
```

Under `[features]`:

```toml
compression = ["zstd"]
```

- [ ] **Step 2: Add compression methods to `RpgStore`**

In `rpg-encoder/src/storage/store.rs`, add:

```rust
#[cfg(feature = "compression")]
impl RpgStore {
    pub fn compress_base(&mut self) -> Result<()> {
        let base_path = self.root.join(BASE_FILE);
        if !base_path.exists() {
            return Err(RpgError::Incremental("No base file to compress".to_string()));
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

        let _ = std::fs::remove_file(&backup);

        Ok(())
    }
}
```

Update the `load()` method to handle compressed base:

```rust
pub fn load(&self) -> Result<RpgSnapshot> {
    let base_path = if self.manifest.base.compressed {
        self.root.join("base.json.zst")
    } else {
        self.root.join(BASE_FILE)
    };

    if !base_path.exists() {
        return Err(RpgError::Incremental("No base file found".to_string()));
    }

    let base_json = if self.manifest.base.compressed {
        #[cfg(feature = "compression")]
        {
            let compressed = std::fs::read(&base_path)?;
            zstd::decode_all(&compressed[..])
                .map_err(|e| RpgError::Incremental(format!("zstd decompression failed: {}", e)))?
        }
        #[cfg(not(feature = "compression"))]
        {
            return Err(RpgError::Incremental(
                "Base is compressed but 'compression' feature is not enabled".to_string(),
            ));
        }
    } else {
        std::fs::read_to_string(&base_path)?
    };

    let base_str = String::from_utf8(base_json)
        .map_err(|e| RpgError::Incremental(format!("Base is not valid UTF-8: {}", e)))?;
    let base: BaseSnapshot = serde_json::from_str(&base_str)
        .map_err(|e| RpgError::Incremental(format!("Failed to parse base: {}", e)))?;

    // ... rest unchanged
}
```

- [ ] **Step 3: Add compression tests**

```rust
#[cfg(feature = "compression")]
#[test]
fn test_compress_and_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path();

    let mut store = RpgStore::init(repo_path).unwrap();

    let mut snapshot = RpgSnapshot::new("test", repo_path);
    snapshot.graph.add_node(
        Node::new(NodeId::new(0), NodeCategory::Function, "function", "rust", "foo"),
    );
    store.save_base(&snapshot).unwrap();

    store.compress_base().unwrap();
    assert!(store.manifest().base.compressed);
    assert!(!repo_path.join(".rpg/base.json").exists());
    assert!(repo_path.join(".rpg/base.json.zst").exists());

    let loaded = store.load().unwrap();
    assert_eq!(loaded.graph.node_count(), 1);
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p rpg-encoder --features compression`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add rpg-encoder/Cargo.toml rpg-encoder/src/storage/store.rs
git commit -m "feat(storage): add optional zstd compression for base file"
```

---

### Task 7: Property tests with proptest

**Files:**
- Create: `rpg-encoder/tests/storage/prop_test.rs`
- Modify: `rpg-encoder/Cargo.toml` — add `[[test]]` entry

- [ ] **Step 1: Write property test for save/load roundtrip**

In `rpg-encoder/tests/storage/prop_test.rs`:

```rust
use proptest::prelude::*;
use rpg_encoder::*;
use rpg_encoder::storage::RpgStore;
use std::path::PathBuf;
use tempfile::TempDir;

proptest! {
    #[test]
    fn save_load_roundtrip_preserves_graph(
        node_count in 0usize..50,
        edge_count in 0usize..100,
    ) {
        let dir = TempDir::new().unwrap();
        let repo_path = dir.path();

        let mut graph = RpgGraph::new();
        let mut node_ids = Vec::new();

        for i in 0..node_count {
            let node = Node::new(
                NodeId::new(0),
                NodeCategory::Function,
                "function",
                "rust",
                format!("fn_{}", i),
            ).with_path(PathBuf::from(format!("src/mod_{}.rs", i % 5)));
            node_ids.push(graph.add_node(node));
        }

        for i in 0..edge_count.min(node_ids.len().saturating_sub(1)) {
            graph.add_typed_edge(node_ids[i], node_ids[i + 1], EdgeType::Calls);
        }

        let mut snapshot = RpgSnapshot::new("prop-test", repo_path);
        snapshot.graph = graph;

        let mut store = RpgStore::init(repo_path).unwrap();
        store.save_base(&snapshot).unwrap();

        let loaded = store.load().unwrap();

        prop_assert_eq!(loaded.graph.node_count(), node_count);
        prop_assert_eq!(loaded.graph.edge_count(), edge_count.min(node_ids.saturating_sub(1)));
    }

    #[test]
    fn patch_apply_preserves_changes(
        nodes_before in 1usize..10,
        nodes_after in 1usize..10,
    ) {
        let dir = TempDir::new().unwrap();
        let repo_path = dir.path();

        let mut graph = RpgGraph::new();
        for i in 0..nodes_before {
            graph.add_node(
                Node::new(NodeId::new(0), NodeCategory::Function, "function", "rust", format!("old_{}", i)),
            );
        }

        let mut snapshot = RpgSnapshot::new("test", repo_path);
        snapshot.graph = graph;

        let mut store = RpgStore::init(repo_path).unwrap();
        store.save_base(&snapshot).unwrap();

        let mut patch = rpg_encoder::storage::Patch::new(1, 0);
        let file_patch = rpg_encoder::storage::FilePatch {
            old_hash: "old".to_string(),
            new_hash: "new".to_string(),
            removed_node_ids: (0..nodes_before).map(|i| format!("node_{}", i)).collect(),
            added_nodes: (0..nodes_after).map(|i| rpg_encoder::encoder::output::SerializedNode {
                id: format!("node_{}", i + 100),
                category: "function".to_string(),
                kind: "function".to_string(),
                language: "rust".to_string(),
                name: format!("new_{}", i),
                path: Some("src/main.rs".to_string()),
                location: None,
                metadata: std::collections::HashMap::new(),
            }).collect(),
            removed_edges: Vec::new(),
            added_edges: Vec::new(),
        };
        patch.changes.modified_files.insert("src/main.rs".to_string(), file_patch);

        store.write_patch(patch).unwrap();

        let loaded = store.load().unwrap();
        prop_assert_eq!(loaded.graph.node_count(), nodes_after);
    }
}
```

Add to `rpg-encoder/Cargo.toml`:

```toml
[[test]]
name = "storage_prop"
path = "tests/storage/prop_test.rs"
```

- [ ] **Step 2: Run property tests**

Run: `cargo test -p rpg-encoder --test storage_prop`
Expected: All proptest cases PASS

- [ ] **Step 3: Commit**

```bash
git add rpg-encoder/tests/storage/ rpg-encoder/Cargo.toml
git commit -m "test(storage): add property tests for save/load roundtrip and patch application"
```

---

### Task 8: Documentation and final verification

**Files:**
- Modify: `rpg-encoder/src/storage/mod.rs` — add module-level docs
- Modify: `rpg-encoder/README.md` — add storage section (optional, only if user requests)

- [ ] **Step 1: Add module-level documentation to `storage/mod.rs`**

```rust
//! Chunked JSON storage with global patch layer.
//!
//! Provides `RpgStore` for persisting RPG graphs as a base JSON file
//! with append-only patch files. Supports automatic compaction and
//! optional zstd compression.
//!
//! # Directory Layout
//!
//! ```text
//! .rpg/
//! ├── manifest.json
//! ├── base.json
//! └── patches/
//!     ├── 001.json
//!     └── 002.json
//! ```
//!
//! # Example
//!
//! ```ignore
//! use rpg_encoder::storage::RpgStore;
//! use rpg_encoder::RpgSnapshot;
//! use std::path::Path;
//!
//! // Initialize
//! let mut store = RpgStore::init(Path::new("./my-project"))?;
//!
//! // Save base
//! store.save_base(&snapshot)?;
//!
//! // Write patch
//! let patch = store.capture_patch(&before, &after, &file_diff);
//! store.write_patch(patch)?;
//!
//! // Load (base + all patches)
//! let loaded = store.load()?;
//!
//! // Compact when needed
//! if store.should_compact() {
//!     store.compact()?;
//! }
//! ```
```

- [ ] **Step 2: Run full test suite (all features)**

Run: `cargo test -p rpg-encoder`
Run: `cargo test -p rpg-encoder --features compression`
Expected: All tests PASS

- [ ] **Step 3: Run clippy**

Run: `cargo clippy -p rpg-encoder -- -D warnings`
Expected: No warnings

- [ ] **Step 4: Commit**

```bash
git add rpg-encoder/src/storage/mod.rs
git commit -m "docs(storage): add module-level documentation"
```
