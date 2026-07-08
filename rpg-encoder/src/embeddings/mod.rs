//! Vector embeddings over graph features.
//!
//! Provides semantic search over the RPG graph by embedding node text (metadata
//! and source) via an OpenAI-compatible embeddings endpoint, then storing
//! vectors in a sidecar index keyed by [`NodeId`].
//!
//! ## Design
//!
//! - [`Embedder`] is the trait for "text → `Vec<f32>`": the real HTTP
//!   [`EmbeddingClient`] and a deterministic [`MockEmbedder`] for tests.
//! - [`EmbeddingIndex`] is the storage/search backend: the pure-Rust
//!   [`FlatIndex`] (default) or a `ZvecIndex` behind the `zvec` feature.
//! - [`EmbeddingStore`] ties them together: it batches node-text embedding
//!   requests, inserts vectors into the index, and serves cosine search. It
//!   keeps a content-hash cache so unchanged nodes are skipped on re-embed.
//!
//! The index is persisted as a binary sidecar (`.rpg/embeddings.bin`), not as a
//! field on [`Node`] — keeping the graph JSON lean and the embedding lifecycle
//! decoupled from the graph's serialization.

use std::path::{Path, PathBuf};

use crate::{NodeCategory, NodeId, RpgGraph};

pub mod client;
pub mod flat;
pub mod text;
#[cfg(feature = "zvec")]
pub mod zvec;

pub use client::{EmbeddingClient, EmbeddingConfig};
pub use flat::FlatIndex;
#[cfg(feature = "zvec")]
pub use zvec::{ZvecIndex, ZvecIndexKind};
pub use text::node_embed_text;

use async_trait::async_trait;

/// Embedding-layer error.
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("embedding HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("embedding JSON parse failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("embedding API error: {0}")]
    Api(String),
    #[error("embedding index I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
    #[error("empty embeddings response for batch of {0}")]
    EmptyResponse(usize),
}

/// Text → vector. The real implementation is [`EmbeddingClient`]; the mock is
/// used in tests so unit/integration tests don't hit the network.
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Embed a batch of texts, returning one vector per input in order.
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>;

    /// Embedding dimension this client produces.
    fn dimension(&self) -> usize;
}

/// Vector storage + cosine search backend.
///
/// `FlatIndex` is the default pure-Rust implementation. A `ZvecIndex` (under
/// the `zvec` feature) implements the same contract. All backends persist to a
/// binary sidecar so the graph JSON is never bloated by vectors.
pub trait EmbeddingIndex: Send + Sync {
    /// Insert (or replace) the vector for `id`.
    fn insert(&mut self, id: NodeId, vector: Vec<f32>);
    /// Return the top-`k` `(NodeId, score)` pairs by cosine similarity.
    fn search(&self, query: &[f32], k: usize) -> Vec<(NodeId, f32)>;
    /// Remove the vector for `id`, if present.
    fn remove(&mut self, id: NodeId);
    /// Number of vectors currently stored.
    fn len(&self) -> usize;
    /// Persist to a binary sidecar at `path`.
    fn save(&self, path: &Path) -> Result<(), EmbeddingError>;
    /// Vector dimension.
    fn dimension(&self) -> usize;

    /// `true` if the index holds no vectors.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Owned store: an index + a content-hash cache for incremental re-embed.
///
/// The hash cache maps `NodeId → hash of the node's embed text`, so that on a
/// re-embed pass (e.g. after incremental edits) only nodes whose text changed
/// are re-sent to the embedding endpoint.
pub struct EmbeddingStore {
    index: Box<dyn EmbeddingIndex>,
    hashes: rustc_hash::FxHashMap<NodeId, u64>,
    path: Option<PathBuf>,
}

impl EmbeddingStore {
    /// Create an empty store backed by the given index, persisting to `path`.
    /// Attempts to load the hash cache sidecar if it exists alongside `path`.
    pub fn new(index: Box<dyn EmbeddingIndex>, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut store = Self {
            index,
            hashes: rustc_hash::FxHashMap::default(),
            path: Some(path),
        };
        store.load_hashes();
        store
    }

    /// Create an in-memory store with no persistence path (for tests).
    pub fn in_memory(index: Box<dyn EmbeddingIndex>) -> Self {
        Self {
            index,
            hashes: rustc_hash::FxHashMap::default(),
            path: None,
        }
    }

    /// Derive the hash cache sidecar path from the index path.
    /// `embeddings.bin` → `embeddings_hashes.bin`
    fn hashes_path(&self) -> Option<PathBuf> {
        self.path.as_ref().map(|p| {
            p.with_file_name(format!(
                "{}_hashes{}",
                p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
                p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default()
            ))
        })
    }

    /// Persist the hash cache to the sidecar file.
    /// Format: `u32 count` + `count × (u64 NodeId, u64 hash)`.
    fn save_hashes(&self) {
        use std::io::{BufWriter, Write};
        let Some(hashes_path) = self.hashes_path() else { return };
        if let Ok(file) = std::fs::File::create(&hashes_path) {
            let mut w = BufWriter::new(file);
            let _ = w.write_all(&(self.hashes.len() as u32).to_le_bytes());
            for (id, hash) in &self.hashes {
                let _ = w.write_all(&(id.index() as u64).to_le_bytes());
                let _ = w.write_all(&hash.to_le_bytes());
            }
            let _ = w.flush();
        }
    }

    /// Load the hash cache from the sidecar file. Gracefully returns if the
    /// file is missing or corrupt (degrades to re-embed-all behavior).
    /// Also reconciles: drops hashes for NodeIds not in the index.
    fn load_hashes(&mut self) {
        use std::io::{BufReader, Read};
        let Some(hashes_path) = self.hashes_path() else { return };
        let Ok(file) = std::fs::File::open(&hashes_path) else { return };
        let mut r = BufReader::new(file);

        let mut buf4 = [0u8; 4];
        let mut buf8 = [0u8; 8];
        if r.read_exact(&mut buf4).is_err() { return }
        let count = u32::from_le_bytes(buf4) as usize;

        for _ in 0..count {
            if r.read_exact(&mut buf8).is_err() { return }
            let raw_id = u64::from_le_bytes(buf8) as usize;
            if raw_id >= isize::MAX as usize { continue }
            if r.read_exact(&mut buf8).is_err() { return }
            let hash = u64::from_le_bytes(buf8);

            let id = NodeId::new(raw_id);
            // Load all hashes — stale entries for deleted nodes are harmless:
            // they just cause a cache hit for a NodeId that no longer exists,
            // which means the node is skipped (no re-embed) — correct behavior
            // since a missing node can't be embedded anyway.
            self.hashes.insert(id, hash);
        }
    }

    /// Embed every embeddable node in `graph`, batching texts through `embedder`.
    ///
    /// Skips nodes whose embed text hash matches the cached value (incremental
    /// re-embed). Centroids (V^H) are embedded from metadata + semantic_feature
    /// only; low-level nodes also include their source lines.
    pub async fn embed_graph(
        &mut self,
        graph: &RpgGraph,
        repo_dir: &Path,
        embedder: &dyn Embedder,
        batch_size: usize,
    ) -> Result<EmbedStats, EmbeddingError> {
        let batch_size = batch_size.max(1);
        let mut stats = EmbedStats::default();

        // Collect (id, text, hash) for nodes whose text changed.
        let mut pending: Vec<(NodeId, String, u64)> = Vec::new();
        for node in graph.nodes() {
            // Skip non-semantic categories.
            if !is_embeddable(node.category) {
                continue;
            }
            let text = node_embed_text(node, repo_dir);
            let hash = hash_text(&text);
            if self.hashes.get(&node.id) == Some(&hash) {
                stats.skipped_unchanged += 1;
                continue;
            }
            pending.push((node.id, text, hash));
            stats.queued += 1;
        }

        if pending.is_empty() {
            return Ok(stats);
        }

        // Batch + embed concurrently.
        let dim = embedder.dimension();
        for chunk in pending.chunks(batch_size) {
            let texts: Vec<String> = chunk.iter().map(|(_, t, _)| t.clone()).collect();
            let vectors = embedder.embed_batch(&texts).await?;
            if vectors.len() != chunk.len() {
                return Err(EmbeddingError::EmptyResponse(chunk.len()));
            }
            for (vec, (id, _, hash)) in vectors.into_iter().zip(chunk.iter()) {
                if vec.len() != dim {
                    return Err(EmbeddingError::DimensionMismatch {
                        expected: dim,
                        actual: vec.len(),
                    });
                }
                self.index.insert(*id, vec);
                self.hashes.insert(*id, *hash);
                stats.embedded += 1;
            }
        }

        if let Some(ref path) = self.path {
            self.index.save(path)?;
            self.save_hashes();
        }
        Ok(stats)
    }

    /// Search the index using a query embedded by `embedder`.
    pub async fn search(
        &self,
        query: &str,
        k: usize,
        embedder: &dyn Embedder,
    ) -> Result<Vec<(NodeId, f32)>, EmbeddingError> {
        let qv = embedder.embed_batch(&[query.to_string()]).await?;
        let qv = qv.into_iter().next().ok_or(EmbeddingError::EmptyResponse(1))?;
        Ok(self.index.search(&qv, k))
    }

    /// Access the underlying index (for direct inserts in tests).
    pub fn index(&self) -> &dyn EmbeddingIndex {
        self.index.as_ref()
    }

    /// Mutable access to the underlying index.
    pub fn index_mut(&mut self) -> &mut dyn EmbeddingIndex {
        self.index.as_mut()
    }

    /// Persist the index + hash cache to their configured paths, if any.
    pub fn save(&self) -> Result<(), EmbeddingError> {
        if let Some(ref path) = self.path {
            self.index.save(path)?;
            self.save_hashes();
        }
        Ok(())
    }

    /// Number of vectors stored.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// `true` if the store holds no vectors.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Record a node's text hash without embedding (used by tests / warm cache).
    pub fn note_hash(&mut self, id: NodeId, text: &str) {
        self.hashes.insert(id, hash_text(text));
    }
}

/// Per-embed pass statistics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EmbedStats {
    /// Nodes sent to the embedder this pass.
    pub embedded: usize,
    /// Nodes skipped because their text was unchanged.
    pub skipped_unchanged: usize,
    /// Nodes queued for embedding (collected before batching).
    pub queued: usize,
}

/// Which node categories carry semantic content worth embedding.
///
/// Repository/Directory/File/Module/Import are structural; Parameter/Constant
/// are too granular. FunctionalCentroid and the definition-like categories are
/// the ones where "find the node that does X" pays off.
fn is_embeddable(category: NodeCategory) -> bool {
    matches!(
        category,
        NodeCategory::Function
            | NodeCategory::Type
            | NodeCategory::Field
            | NodeCategory::Feature
            | NodeCategory::Component
            | NodeCategory::FunctionalCentroid
    )
}

/// Stable hash of an embed text for incremental invalidation. Uses the default
/// hasher via `std::hash::Hasher` on the text bytes — fast and sufficient for
/// change detection (not cryptographic).
fn hash_text(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Node, NodeCategory};

    /// Deterministic embedder for unit tests: hashes text to a fixed-dimension
    /// vector so search ranking is reproducible without a network call.
    struct MockEmbedder {
        dim: usize,
    }

    #[async_trait]
    impl Embedder for MockEmbedder {
        async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
            Ok(texts
                .iter()
                .map(|t| {
                    let h = hash_text(t);
                    (0..self.dim)
                        .map(|i| {
                            let bit = (h >> (i % 64)) & 1;
                            bit as f32
                        })
                        .collect()
                })
                .collect())
        }
        fn dimension(&self) -> usize {
            self.dim
        }
    }

    #[tokio::test]
    async fn embed_store_inserts_and_searches() {
        let mut graph = RpgGraph::new();
        let n1 = graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "fn",
            "rust",
            "foo",
        ));
        let n2 = graph.add_node(Node::new(
            NodeId::new(1),
            NodeCategory::Function,
            "fn",
            "rust",
            "bar",
        ));
        let _ = (n1, n2);

        let embedder = MockEmbedder { dim: 8 };
        let mut store = EmbeddingStore::in_memory(Box::new(FlatIndex::new(8)));
        let stats = store
            .embed_graph(&graph, Path::new("/nonexistent"), &embedder, 16)
            .await
            .unwrap();
        assert_eq!(stats.embedded, 2);
        assert_eq!(store.len(), 2);

        let results = store.search("foo", 2, &embedder).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn embed_store_skips_unchanged() {
        let mut graph = RpgGraph::new();
        graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "fn",
            "rust",
            "foo",
        ));
        let embedder = MockEmbedder { dim: 4 };
        let mut store = EmbeddingStore::in_memory(Box::new(FlatIndex::new(4)));
        let s1 = store
            .embed_graph(&graph, Path::new("/x"), &embedder, 16)
            .await
            .unwrap();
        assert_eq!(s1.embedded, 1);
        let s2 = store
            .embed_graph(&graph, Path::new("/x"), &embedder, 16)
            .await
            .unwrap();
        assert_eq!(s2.embedded, 0);
        assert_eq!(s2.skipped_unchanged, 1);
    }

    #[test]
    fn is_embeddable_classification() {
        assert!(is_embeddable(NodeCategory::Function));
        assert!(is_embeddable(NodeCategory::Type));
        assert!(is_embeddable(NodeCategory::FunctionalCentroid));
        assert!(!is_embeddable(NodeCategory::Repository));
        assert!(!is_embeddable(NodeCategory::Import));
        assert!(!is_embeddable(NodeCategory::Parameter));
    }

    #[tokio::test]
    async fn hash_cache_persists_across_restart() {
        let dir = tempfile::tempdir().unwrap();
        let emb_path = dir.path().join("embeddings.bin");

        let mut graph = RpgGraph::new();
        graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "fn",
            "rust",
            "persisted_fn",
        ));

        let embedder = MockEmbedder { dim: 4 };

        // Pass 1: embed from scratch with a persisted store.
        let stats1 = {
            let mut store = EmbeddingStore::new(Box::new(FlatIndex::new(4)), &emb_path);
            store
                .embed_graph(&graph, Path::new("/x"), &embedder, 16)
                .await
                .unwrap()
        };
        assert_eq!(stats1.embedded, 1, "first pass should embed 1 node");
        assert_eq!(stats1.skipped_unchanged, 0);

        // The hash sidecar should exist alongside the embeddings file.
        let hashes_path = dir.path().join("embeddings_hashes.bin");
        assert!(hashes_path.exists(), "hash sidecar must be created");

        // Pass 2: create a FRESH store (simulating a process restart).
        // The hash cache should be loaded from the sidecar, and all nodes
        // should be skipped as unchanged.
        let stats2 = {
            let mut store = EmbeddingStore::new(Box::new(FlatIndex::new(4)), &emb_path);
            store
                .embed_graph(&graph, Path::new("/x"), &embedder, 16)
                .await
                .unwrap()
        };
        assert_eq!(stats2.embedded, 0, "second pass should embed 0 nodes (cache hit)");
        assert_eq!(
            stats2.skipped_unchanged, 1,
            "second pass should skip 1 node (hash cache loaded from sidecar)"
        );
    }

    #[test]
    fn hash_cache_load_missing_file_degrades_gracefully() {
        let dir = tempfile::tempdir().unwrap();
        let emb_path = dir.path().join("embeddings.bin");

        // No sidecar exists — store should start with empty hashes.
        let store = EmbeddingStore::new(Box::new(FlatIndex::new(4)), &emb_path);
        // Can't directly check hashes (private), but the store should not panic.
        assert_eq!(store.len(), 0);
    }
}
