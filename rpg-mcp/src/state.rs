use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use rpg_encoder::{RpgGraph, RpgSnapshot, RpgStore};
use sha2::{Digest, Sha256};

pub fn load_dotenv() {
    if let Ok(path) = std::env::var("RPG_ENV_FILE") {
        let _ = dotenvy::from_path_override(&path);
        return;
    }

    let candidates: Vec<PathBuf> = [
        std::env::var("CARGO_MANIFEST_DIR").ok().map(PathBuf::from),
        std::env::var("RPG_WORKSPACE").ok().map(PathBuf::from),
        Some(std::env::current_dir().unwrap_or_default()),
    ]
    .into_iter()
    .flatten()
    .flat_map(|dir| {
        [dir.parent().map(|p| p.join(".env")), Some(dir.join(".env"))]
            .into_iter()
            .flatten()
    })
    .collect();

    for path in candidates {
        if path.exists() && dotenvy::from_path_override(&path).is_ok() {
            return;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashMode {
    Mtime,
    Content,
}

impl HashMode {
    fn from_str(s: &str) -> Self {
        match s {
            "content" => Self::Content,
            _ => Self::Mtime,
        }
    }
}

#[derive(Debug, Clone)]
pub struct McpConfig {
    pub workspace: PathBuf,
    pub data_dir: PathBuf,
    pub hash_mode: HashMode,
    #[allow(dead_code)]
    pub semantic: bool,
}

impl McpConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        load_dotenv();

        let workspace = std::env::var("RPG_WORKSPACE")
            .map_err(|_| anyhow::anyhow!("RPG_WORKSPACE env var is required"))?;
        let workspace = PathBuf::from(&workspace);

        let data_dir = std::env::var("RPG_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| workspace.join(".rpg-data"));

        let hash_mode = std::env::var("RPG_HASH_MODE")
            .map(|s| HashMode::from_str(&s))
            .unwrap_or(HashMode::Mtime);

        let semantic = std::env::var("RPG_SEMANTIC")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false);

        Ok(Self {
            workspace,
            data_dir,
            hash_mode,
            semantic,
        })
    }
}

pub struct AppState {
    pub config: McpConfig,
    pub graph: Arc<RwLock<RpgGraph>>,
    pub snapshot: Arc<RwLock<RpgSnapshot>>,
    pub store: Arc<RwLock<Option<RpgStore>>>,
}

impl AppState {
    pub fn new(config: McpConfig, snapshot: RpgSnapshot) -> Self {
        let graph = snapshot.graph.clone();
        Self {
            config,
            graph: Arc::new(RwLock::new(graph)),
            snapshot: Arc::new(RwLock::new(snapshot)),
            store: Arc::new(RwLock::new(None)),
        }
    }

    pub fn update(&self, new_snapshot: RpgSnapshot) {
        let new_graph = new_snapshot.graph.clone();
        *self.graph.write().expect("graph lock poisoned") = new_graph;
        *self.snapshot.write().expect("snapshot lock poisoned") = new_snapshot;
    }
}

pub fn compute_dir_hash(dir: &Path, mode: HashMode) -> anyhow::Result<String> {
    let mut entries: Vec<(PathBuf, String)> = Vec::new();

    for entry in walkdir::WalkDir::new(dir).follow_links(false).into_iter() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let value = match mode {
            HashMode::Mtime => {
                let metadata = std::fs::metadata(path)?;
                metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos().to_string())
                    .unwrap_or_default()
            }
            HashMode::Content => {
                let content = std::fs::read(path)?;
                let mut hasher = Sha256::new();
                hasher.update(&content);
                hex::encode(hasher.finalize())
            }
        };

        let relative = path.strip_prefix(dir).unwrap_or(path).to_path_buf();
        entries.push((relative, value));
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (path, value) in &entries {
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(b"|");
        hasher.update(value.as_bytes());
        hasher.update(b"\n");
    }

    Ok(hex::encode(hasher.finalize()))
}

pub fn load_dir_hash(data_dir: &Path) -> Option<String> {
    let hash_path = data_dir.join(".rpg").join("dir_hash");
    std::fs::read_to_string(&hash_path)
        .ok()
        .map(|s| s.trim().to_string())
}

pub fn save_dir_hash(data_dir: &Path, hash: &str) -> anyhow::Result<()> {
    let rpg_dir = data_dir.join(".rpg");
    std::fs::create_dir_all(&rpg_dir)?;
    let hash_path = rpg_dir.join("dir_hash");
    std::fs::write(&hash_path, hash)?;
    Ok(())
}
