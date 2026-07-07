use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;

use rpg_encoder::{ParserRegistry, RpgGraph, RpgSnapshot, RpgStore};
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
        match s.to_lowercase().as_str() {
            "content" => Self::Content,
            _ => Self::Mtime,
        }
    }
}

/// Parse a boolean env var, case-insensitive. Accepts "true", "1", "yes", "on".
/// Everything else (including unset) is false.
fn parse_bool_env(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => matches!(v.to_lowercase().as_str(), "true" | "1" | "yes" | "on"),
        Err(_) => false,
    }
}

#[derive(Debug, Clone)]
pub struct McpConfig {
    pub workspace: PathBuf,
    pub data_dir: PathBuf,
    pub hash_mode: HashMode,
    pub semantic: bool,
}

impl McpConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        load_dotenv();

        let workspace = std::env::var("RPG_WORKSPACE")
            .map_err(|_| anyhow::anyhow!(
                "RPG_WORKSPACE env var is required. Set it in .env (see .env.example) or as an environment variable."
            ))?;
        let workspace = PathBuf::from(&workspace);

        // Validate workspace exists and is a directory.
        if !workspace.is_dir() {
            return Err(anyhow::anyhow!(
                "RPG_WORKSPACE '{}' does not exist or is not a directory.",
                workspace.display()
            ));
        }

        let data_dir = std::env::var("RPG_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| workspace.join(".rpg"));

        let hash_mode = std::env::var("RPG_HASH_MODE")
            .map(|s| HashMode::from_str(&s))
            .unwrap_or(HashMode::Mtime);

        let semantic = parse_bool_env("RPG_SEMANTIC");

        // Warn if semantic mode is enabled but no API key is set.
        if semantic && std::env::var("OPENAI_API_KEY").is_err() {
            tracing::warn!(
                "RPG_SEMANTIC=true but OPENAI_API_KEY is not set. \
                 Semantic enrichment will fail when encoding. \
                 Set OPENAI_API_KEY in .env or disable RPG_SEMANTIC."
            );
        }

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
    /// Parser registry shared between the watcher and MCP tools (e.g.
    /// detect_changes). Read-only after initialization.
    pub registry: Arc<ParserRegistry>,
    /// Vector embedding index (sidecar at `<workspace>/.rpg/embeddings.bin`),
    /// loaded lazily by `vector_search`. `None` when no embeddings have been
    /// computed yet (run `encode_with_embeddings` first).
    pub embeddings: Arc<RwLock<Option<rpg_encoder::FlatIndex>>>,
    /// Agent memory store (cross-session notes). Loaded lazily on first use.
    pub memories: Arc<crate::tools::memory::MemoryStore>,
}

impl AppState {
    pub fn new(config: McpConfig, snapshot: RpgSnapshot, registry: Arc<ParserRegistry>) -> Self {
        let graph = snapshot.graph.clone();
        let memories = Arc::new(crate::tools::memory::MemoryStore::from_env(&config.workspace));
        Self {
            config,
            graph: Arc::new(RwLock::new(graph)),
            snapshot: Arc::new(RwLock::new(snapshot)),
            store: Arc::new(RwLock::new(None)),
            registry,
            embeddings: Arc::new(RwLock::new(None)),
            memories,
        }
    }

    pub fn update(&self, new_snapshot: RpgSnapshot) {
        let new_graph = new_snapshot.graph.clone();
        *self.graph.write() = new_graph;
        *self.snapshot.write() = new_snapshot;
    }
}

pub fn compute_dir_hash(dir: &Path, mode: HashMode) -> anyhow::Result<String> {
    let mut entries: Vec<(PathBuf, String)> = Vec::new();

    // Skip directories that change independently of source code — including
    // them would make the hash change on every save/compile, defeating the
    // detect_changes fast path.
    let skip_dirs: &[&str] = &[".rpg", ".git", "target", "node_modules", ".next", "dist", "build"];

    for entry in walkdir::WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if !e.file_type().is_dir() {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !skip_dirs.contains(&name.as_ref())
        })
    {
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
    // data_dir IS the .rpg directory now — the hash file lives directly in it.
    let hash_path = data_dir.join("dir_hash");
    std::fs::read_to_string(&hash_path)
        .ok()
        .map(|s| s.trim().to_string())
}

pub fn save_dir_hash(data_dir: &Path, hash: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let hash_path = data_dir.join("dir_hash");
    std::fs::write(&hash_path, hash)?;
    Ok(())
}
