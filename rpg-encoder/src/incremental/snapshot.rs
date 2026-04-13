use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::hash::compute_file_hash;
use crate::core::{EdgeType, NodeCategory, NodeId, RpgGraph};
use crate::encoder::RpgEncoder;
use crate::error::{Result, RpgError};

pub const SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnitType {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Module,
}

impl UnitType {
    pub fn from_kind(kind: &str) -> Option<Self> {
        match kind {
            "function" => Some(UnitType::Function),
            "struct" => Some(UnitType::Struct),
            "enum" => Some(UnitType::Enum),
            "trait" => Some(UnitType::Trait),
            "impl" => Some(UnitType::Impl),
            "module" => Some(UnitType::Module),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedUnit {
    pub name: String,
    pub unit_type: UnitType,
    pub content_hash: String,
    pub start_line: usize,
    pub end_line: usize,
    pub features: Vec<String>,
    pub description: String,
    pub node_id: Option<NodeId>,
    pub modified_at: u64,
}

impl CachedUnit {
    pub fn new(
        name: String,
        unit_type: UnitType,
        content_hash: String,
        start_line: usize,
        end_line: usize,
    ) -> Self {
        Self {
            name,
            unit_type,
            content_hash,
            start_line,
            end_line,
            features: Vec::new(),
            description: String::new(),
            node_id: None,
            modified_at: current_timestamp(),
        }
    }

    pub fn with_features(mut self, features: Vec<String>) -> Self {
        self.features = features;
        self
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    pub fn with_node_id(mut self, node_id: NodeId) -> Self {
        self.node_id = Some(node_id);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpgSnapshot {
    pub version: u32,
    pub repo_name: String,
    pub repo_info: String,
    pub repo_dir: PathBuf,
    pub graph: RpgGraph,
    pub file_hashes: HashMap<PathBuf, String>,
    pub unit_cache: HashMap<PathBuf, Vec<CachedUnit>>,
    #[serde(skip)]
    pub node_id_index: HashMap<NodeId, (PathBuf, usize)>,
    pub reverse_deps: HashMap<NodeId, Vec<NodeId>>,
    pub excluded_files: Vec<PathBuf>,
    pub encoding_timestamp: u64,
    pub last_modified: u64,
}

impl RpgSnapshot {
    pub fn new(repo_name: &str, repo_dir: &Path) -> Self {
        let now = current_timestamp();
        Self {
            version: SNAPSHOT_VERSION,
            repo_name: repo_name.to_string(),
            repo_info: String::new(),
            repo_dir: repo_dir.to_path_buf(),
            graph: RpgGraph::new(),
            file_hashes: HashMap::new(),
            unit_cache: HashMap::new(),
            node_id_index: HashMap::new(),
            reverse_deps: HashMap::new(),
            excluded_files: Vec::new(),
            encoding_timestamp: now,
            last_modified: now,
        }
    }

    pub fn from_encoder(encoder: &RpgEncoder) -> Self {
        let graph = encoder.graph().cloned().unwrap_or_default();
        let repo_dir = encoder.root().map(PathBuf::from).unwrap_or_default();
        let repo_name = repo_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repository")
            .to_string();

        let now = current_timestamp();
        Self {
            version: SNAPSHOT_VERSION,
            repo_name,
            repo_info: String::new(),
            repo_dir,
            graph,
            file_hashes: HashMap::new(),
            unit_cache: HashMap::new(),
            node_id_index: HashMap::new(),
            reverse_deps: HashMap::new(),
            excluded_files: Vec::new(),
            encoding_timestamp: now,
            last_modified: now,
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(RpgError::JsonError)?;
        std::fs::write(path, json)?;
        tracing::info!("Saved snapshot to {}", path.display());
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let mut snapshot: RpgSnapshot = serde_json::from_str(&json).map_err(RpgError::JsonError)?;

        if snapshot.version != SNAPSHOT_VERSION {
            return Err(RpgError::Incremental(format!(
                "Incompatible snapshot version: {} (expected {})",
                snapshot.version, SNAPSHOT_VERSION
            )));
        }

        tracing::info!(
            "Loaded snapshot from {} ({} nodes, {} edges)",
            path.display(),
            snapshot.graph.node_count(),
            snapshot.graph.edge_count()
        );

        snapshot.rebuild_node_id_index();

        Ok(snapshot)
    }

    pub fn compute_file_hashes(&mut self) -> Result<()> {
        self.file_hashes.clear();

        for node in self.graph.nodes() {
            if node.category == NodeCategory::File {
                if let Some(path) = &node.path {
                    let full_path = self.repo_dir.join(path);
                    if full_path.exists() {
                        let hash = compute_file_hash(&full_path)?;
                        self.file_hashes.insert(path.clone(), hash);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn build_reverse_deps(&mut self) {
        self.reverse_deps.clear();

        for node in self.graph.nodes() {
            self.reverse_deps.entry(node.id).or_default();
        }

        for (source, target, edge) in self.graph.edges() {
            if matches!(
                edge.edge_type,
                EdgeType::Calls
                    | EdgeType::UsesType
                    | EdgeType::References
                    | EdgeType::Extends
                    | EdgeType::Implements
            ) {
                self.reverse_deps.entry(target).or_default().push(source);
            }
        }
    }

    pub fn dependents_of(&self, node_id: NodeId) -> Vec<NodeId> {
        self.reverse_deps.get(&node_id).cloned().unwrap_or_default()
    }

    pub fn update_timestamp(&mut self) {
        self.last_modified = current_timestamp();
    }

    pub fn get_units_for_file(&self, file_path: &Path) -> Option<&[CachedUnit]> {
        self.unit_cache.get(file_path).map(|v| v.as_slice())
    }

    pub fn get_unit_for_node(&self, node_id: NodeId) -> Option<(&PathBuf, &CachedUnit)> {
        self.node_id_index.get(&node_id).and_then(|(path, idx)| {
            self.unit_cache
                .get(path)
                .and_then(|units| units.get(*idx))
                .map(|unit| (path, unit))
        })
    }

    pub fn insert_units(&mut self, file_path: PathBuf, units: Vec<CachedUnit>) {
        for (idx, unit) in units.iter().enumerate() {
            if let Some(node_id) = unit.node_id {
                self.node_id_index.insert(node_id, (file_path.clone(), idx));
            }
        }
        self.unit_cache.insert(file_path, units);
    }

    pub fn rebuild_node_id_index(&mut self) {
        self.node_id_index.clear();
        for (file_path, units) in &self.unit_cache {
            for (idx, unit) in units.iter().enumerate() {
                if let Some(node_id) = unit.node_id {
                    self.node_id_index.insert(node_id, (file_path.clone(), idx));
                }
            }
        }
    }

    pub fn stats(&self) -> SnapshotStats {
        SnapshotStats {
            version: self.version,
            node_count: self.graph.node_count(),
            edge_count: self.graph.edge_count(),
            file_count: self.file_hashes.len(),
            cached_units: self.unit_cache.values().map(|v| v.len()).sum(),
            reverse_dep_entries: self.reverse_deps.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotStats {
    pub version: u32,
    pub node_count: usize,
    pub edge_count: usize,
    pub file_count: usize,
    pub cached_units: usize,
    pub reverse_dep_entries: usize,
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{EdgeType, Node, NodeCategory, NodeId};
    use std::path::Path;

    #[test]
    fn test_snapshot_save_load_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file_path = dir.path().join("snapshot.json");

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

        snapshot.unit_cache.insert(
            PathBuf::from("src/main.rs"),
            vec![CachedUnit::new(
                "main".to_string(),
                UnitType::Function,
                "abc123".to_string(),
                1,
                5,
            )],
        );

        snapshot.save(&file_path).expect("save");
        assert!(file_path.exists());

        let loaded = RpgSnapshot::load(&file_path).expect("load");
        assert_eq!(loaded.repo_name, "test-repo");
        assert_eq!(loaded.version, SNAPSHOT_VERSION);
        assert_eq!(loaded.graph.node_count(), 1);
        assert_eq!(loaded.unit_cache.len(), 1);
    }

    #[test]
    fn test_snapshot_new_defaults() {
        let snapshot = RpgSnapshot::new("my-repo", Path::new("/fake"));
        assert_eq!(snapshot.repo_name, "my-repo");
        assert_eq!(snapshot.version, SNAPSHOT_VERSION);
        assert_eq!(snapshot.graph.node_count(), 0);
        assert!(snapshot.file_hashes.is_empty());
        assert!(snapshot.unit_cache.is_empty());
        assert!(snapshot.reverse_deps.is_empty());
    }

    #[test]
    fn test_cached_unit_builders() {
        let unit = CachedUnit::new(
            "my_fn".to_string(),
            UnitType::Function,
            "hash1".to_string(),
            10,
            20,
        )
        .with_features(vec!["auth".to_string()])
        .with_description("does stuff".to_string())
        .with_node_id(NodeId::new(42));

        assert_eq!(unit.name, "my_fn");
        assert_eq!(unit.unit_type, UnitType::Function);
        assert_eq!(unit.start_line, 10);
        assert_eq!(unit.end_line, 20);
        assert_eq!(unit.features, vec!["auth"]);
        assert_eq!(unit.description, "does stuff");
        assert_eq!(unit.node_id, Some(NodeId::new(42)));
    }

    #[test]
    fn test_unit_type_from_kind() {
        assert_eq!(UnitType::from_kind("function"), Some(UnitType::Function));
        assert_eq!(UnitType::from_kind("struct"), Some(UnitType::Struct));
        assert_eq!(UnitType::from_kind("enum"), Some(UnitType::Enum));
        assert_eq!(UnitType::from_kind("trait"), Some(UnitType::Trait));
        assert_eq!(UnitType::from_kind("impl"), Some(UnitType::Impl));
        assert_eq!(UnitType::from_kind("module"), Some(UnitType::Module));
        assert_eq!(UnitType::from_kind("unknown"), None);
    }

    #[test]
    fn test_build_reverse_deps() {
        let mut snapshot = RpgSnapshot::new("test", Path::new("/tmp"));
        let n1 = snapshot.graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "fn",
            "rust",
            "caller",
        ));
        let n2 = snapshot.graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "fn",
            "rust",
            "callee",
        ));
        snapshot.graph.add_typed_edge(n1, n2, EdgeType::Calls);
        snapshot.build_reverse_deps();

        let deps = snapshot.dependents_of(n2);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0], n1);
    }

    #[test]
    fn test_get_units_for_file() {
        let mut snapshot = RpgSnapshot::new("test", Path::new("/tmp"));
        let path = PathBuf::from("src/lib.rs");
        snapshot.unit_cache.insert(
            path.clone(),
            vec![CachedUnit::new(
                "helper".to_string(),
                UnitType::Function,
                "h".to_string(),
                1,
                3,
            )],
        );

        let units = snapshot.get_units_for_file(&path);
        assert!(units.is_some());
        assert_eq!(units.unwrap().len(), 1);

        let missing = snapshot.get_units_for_file(Path::new("nope.rs"));
        assert!(missing.is_none());
    }

    #[test]
    fn test_snapshot_stats() {
        let mut snapshot = RpgSnapshot::new("test", Path::new("/tmp"));
        snapshot.graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::File,
            "file",
            "rust",
            "f.rs",
        ));

        let stats = snapshot.stats();
        assert_eq!(stats.version, SNAPSHOT_VERSION);
        assert_eq!(stats.node_count, 1);
        assert_eq!(stats.edge_count, 0);
        assert_eq!(stats.file_count, 0);
    }

    #[test]
    fn test_insert_units() {
        let mut snapshot = RpgSnapshot::new("test", Path::new("/tmp"));
        let path = PathBuf::from("src/main.rs");
        let node_id = NodeId::new(7);
        let units = vec![CachedUnit::new(
            "main".to_string(),
            UnitType::Function,
            "h".to_string(),
            1,
            1,
        )
        .with_node_id(node_id)];

        snapshot.insert_units(path.clone(), units);

        let (found_path, found_unit) = snapshot.get_unit_for_node(node_id).expect("found");
        assert_eq!(found_path, &path);
        assert_eq!(found_unit.name, "main");
    }
}
