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
    pub fn from_category(category: NodeCategory) -> Option<Self> {
        match category {
            NodeCategory::Function => Some(UnitType::Function),
            NodeCategory::Type => Some(UnitType::Struct),
            NodeCategory::Module => Some(UnitType::Module),
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
        let snapshot: RpgSnapshot = serde_json::from_str(&json).map_err(RpgError::JsonError)?;

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
        for (file_path, units) in &self.unit_cache {
            for unit in units {
                if unit.node_id == Some(node_id) {
                    return Some((file_path, unit));
                }
            }
        }
        None
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
