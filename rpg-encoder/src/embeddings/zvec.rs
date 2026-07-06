//! zvec-backed embedding index — the alternative to [`FlatIndex`].
//!
//! Wraps `zvec_bindings::Collection` (Alibaba's in-process vector database)
//! behind the same [`EmbeddingIndex`] trait. zvec brings a C++/Proxima native
//! dependency but offers HNSW/IVF indexing for datasets where brute-force cosine
//! stops being fast enough (millions of vectors — well beyond typical code
//! graphs, hence the default [`FlatIndex`]).
//!
//! ## Design notes
//!
//! - Uses `Collection` directly (not `SharedCollection`): `Collection` is
//!   already `Send + Sync` via upstream `unsafe impl`, and only the bare
//!   `Collection` exposes `stats().doc_count()` which backs [`EmbeddingIndex::len`].
//! - The primary key is the NodeId index formatted as a decimal string.
//! - Uses a FLAT index with cosine metric by default, matching the FlatIndex
//!   baseline. Switch to HNSW via [`ZvecIndex::with_hnsw`] for large-scale
//!   benchmarks.
//! - `upsert` is used for insert so re-embedding a node replaces its vector.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use zvec_bindings::collection::Collection;
use zvec_bindings::doc::{Doc, DocList};
use zvec_bindings::query::VectorQuery;
use zvec_bindings::schema::{CollectionSchema, VectorSchema};
use zvec_bindings::{IndexParams, MetricType, QuantizeType};

use crate::NodeId;

use super::{EmbeddingError, EmbeddingIndex};

/// Field name for the dense vector column in the zvec collection.
const VEC_FIELD: &str = "embedding";
/// Collection name (zvec requires one).
const COLLECTION_NAME: &str = "rpg_embeddings";

/// zvec-backed embedding index.
///
/// Wraps a `zvec_bindings::Collection` behind [`EmbeddingIndex`]. The inner
/// `Collection` is `Send + Sync` (upstream `unsafe impl`); we wrap it in a
/// `Mutex` to coordinate mutable sidecar lifecycle operations.
pub struct ZvecIndex {
    dim: usize,
    collection: Mutex<Collection>,
    /// Inline cache of inserted NodeId → pk string, so `len()` and `remove()`
    /// don't require a zvec round-trip.
    pks: Mutex<HashMap<NodeId, String>>,
}

/// Index algorithm to use when creating the zvec collection.
#[derive(Debug, Clone, Copy, Default)]
pub enum ZvecIndexKind {
    /// Brute-force flat index with cosine metric. Matches FlatIndex semantics.
    /// Best for code-graph scale (<100k vectors).
    #[default]
    Flat,
    /// HNSW graph index. Faster search at scale, higher build cost + memory.
    /// Use for benchmarking against FlatIndex at large N.
    Hnsw { m: i32, ef_construction: i32 },
}

impl ZvecIndex {
    /// Create (or open, if `path` exists) a zvec collection with a flat cosine
    /// index of `dim` dimensions.
    ///
    /// # Errors
    /// Returns an error if zvec fails to create/open the collection or build
    /// the index.
    pub fn new(path: &Path, dim: usize) -> Result<Self, EmbeddingError> {
        Self::with_kind(path, dim, ZvecIndexKind::Flat)
    }

    /// Create (or open) with a specific index kind (flat or HNSW).
    ///
    /// Note: zvec creates a default index when the collection is opened. The
    /// `kind` parameter controls whether an explicit flat/HNSW index is created
    /// in addition — for the default [`ZvecIndexKind::Flat`] we let zvec use its
    /// built-in index (which is already flat-cosine for fp32 vectors), avoiding
    /// a redundant `create_index` call. HNSW is created explicitly when requested.
    pub fn with_kind(path: &Path, dim: usize, kind: ZvecIndexKind) -> Result<Self, EmbeddingError> {
        // init() is optional (zvec auto-initializes on first use) but calling
        // it eagerly surfaces init errors at construction rather than mid-embed.
        let _ = zvec_bindings::init();

        let collection = if path.exists() {
            Collection::open(path).map_err(zvec_err)?
        } else {
            let mut schema = CollectionSchema::new(COLLECTION_NAME);
            let vec_field = VectorSchema::fp32(VEC_FIELD, dim as u32);
            schema.add_field(vec_field).map_err(zvec_err)?;
            Collection::create_and_open(path, schema).map_err(zvec_err)?
        };

        // Only create an explicit HNSW index when requested; the default flat
        // index is built into the collection and needs no separate call.
        if let ZvecIndexKind::Hnsw { m, ef_construction } = kind {
            collection
                .create_index(
                    VEC_FIELD,
                    IndexParams::hnsw(m, ef_construction, MetricType::Cosine, QuantizeType::Undefined),
                )
                .map_err(zvec_err)?;
        }

        Ok(Self {
            dim,
            collection: Mutex::new(collection),
            pks: Mutex::new(HashMap::new()),
        })
    }
}

impl EmbeddingIndex for ZvecIndex {
    fn insert(&mut self, id: NodeId, vector: Vec<f32>) {
        let pk = id.index().to_string();
        let mut doc = Doc::id(&pk);
        let _ = doc.set_vector(VEC_FIELD, &vector);

        let coll = self.collection.lock().expect("zvec lock poisoned");
        let _ = coll.upsert(&[doc]);

        self.pks.lock().expect("pks lock poisoned").insert(id, pk);
    }

    fn search(&self, query: &[f32], k: usize) -> Vec<(NodeId, f32)> {
        let coll = self.collection.lock().expect("zvec lock poisoned");
        let vq = match VectorQuery::new(VEC_FIELD).topk(k).vector(query) {
            Ok(q) => q,
            Err(_) => return Vec::new(),
        };
        let results: DocList = match coll.query(vq) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        results
            .iter()
            .filter_map(|doc| {
                let pk = doc.pk();
                let idx: usize = pk.parse().ok()?;
                Some((NodeId::new(idx), doc.score()))
            })
            .collect()
    }

    fn remove(&mut self, id: NodeId) {
        let pk = id.index().to_string();
        let coll = self.collection.lock().expect("zvec lock poisoned");
        let _ = coll.delete(&[pk.as_str()]);
        self.pks.lock().expect("pks lock poisoned").remove(&id);
    }

    fn len(&self) -> usize {
        let coll = self.collection.lock().expect("zvec lock poisoned");
        coll.stats()
            .map(|s| s.doc_count() as usize)
            .unwrap_or_else(|_| self.pks.lock().expect("pks lock poisoned").len())
    }

    fn save(&self, _path: &Path) -> Result<(), EmbeddingError> {
        // zvec persists to its collection directory on every write; the path
        // argument is the collection directory itself. Flush to ensure
        // durability.
        let coll = self.collection.lock().expect("zvec lock poisoned");
        coll.flush().map_err(zvec_err)?;
        Ok(())
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

/// Convert a zvec_bindings error into our EmbeddingError.
fn zvec_err(e: zvec_bindings::error::Error) -> EmbeddingError {
    EmbeddingError::Api(format!("zvec: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a zvec collection in a fresh subdirectory of a temp dir. zvec
    /// manages its own LOCK file inside the path, so the leaf directory must
    /// NOT pre-exist (zvec creates it).
    fn make_index(dim: usize) -> (ZvecIndex, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        // zvec creates the leaf; give it a child path under the temp dir.
        let path = dir.path().join("coll");
        let idx = ZvecIndex::new(&path, dim).expect("zvec collection created");
        (idx, dir)
    }

    #[test]
    fn insert_and_search_returns_match() {
        let (mut idx, _dir) = make_index(4);
        idx.insert(NodeId::new(0), vec![1.0, 0.0, 0.0, 0.0]);
        idx.insert(NodeId::new(1), vec![0.0, 1.0, 0.0, 0.0]);

        let r = idx.search(&[1.0, 0.0, 0.0, 0.0], 2);
        assert!(r.len() >= 1);
        // Top hit should be node 0 (exact cosine = 1.0).
        assert_eq!(r[0].0.index(), 0);
        assert!((r[0].1 - 1.0).abs() < 1e-5);
    }

    #[test]
    fn empty_search_returns_empty() {
        let (idx, _dir) = make_index(4);
        let r = idx.search(&[1.0, 0.0, 0.0, 0.0], 5);
        assert!(r.is_empty());
    }

    #[test]
    fn remove_drops_vector() {
        let (mut idx, _dir) = make_index(4);
        idx.insert(NodeId::new(0), vec![1.0, 0.0, 0.0, 0.0]);
        idx.insert(NodeId::new(1), vec![0.0, 1.0, 0.0, 0.0]);

        idx.remove(NodeId::new(0));
        // Search for the removed vector — node 0 should not be in results.
        let r = idx.search(&[1.0, 0.0, 0.0, 0.0], 5);
        assert!(r.iter().all(|(id, _)| id.index() != 0));
    }

    #[test]
    fn reopen_preserves_vectors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("coll");

        {
            let mut idx = ZvecIndex::new(&path, 4).unwrap();
            idx.insert(NodeId::new(0), vec![1.0, 0.0, 0.0, 0.0]);
            idx.insert(NodeId::new(1), vec![0.0, 1.0, 0.0, 0.0]);
            idx.save(&path).unwrap();
        }

        // Reopen — the collection should retain the inserted vectors.
        let idx = ZvecIndex::new(&path, 4).unwrap();
        let r = idx.search(&[1.0, 0.0, 0.0, 0.0], 2);
        assert!(!r.is_empty(), "reopened index should find results");
    }

    #[test]
    fn dimension_reported_correctly() {
        let (idx, _dir) = make_index(128);
        assert_eq!(idx.dimension(), 128);
    }
}
