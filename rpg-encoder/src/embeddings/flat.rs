//! Pure-Rust brute-force cosine index — the default embedding backend.
//!
//! At code-graph scale (100–10k nodes, 4096 dims) a flat scan is sub-5ms, so an
//! approximate index (HNSW/IVF) buys nothing. This backend has zero native
//! dependencies and a trivial binary persistence format, which keeps the build
//! clean and the footprint small. The `zvec` feature can slot in a faster index
//! if scale ever demands it, behind the same [`EmbeddingIndex`] trait.

use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use crate::NodeId;

use super::{EmbeddingError, EmbeddingIndex};

/// On-disk magic + version for the flat index sidecar.
const MAGIC: &[u8; 4] = b"RPGE";
const VERSION: u16 = 1;

/// Brute-force cosine-similarity index.
///
/// Stores vectors contiguously per node id. Search is O(n·d) followed by a
/// partial top-k selection — fast enough for any realistic code graph.
pub struct FlatIndex {
    dim: usize,
    vectors: HashMap<NodeId, Vec<f32>>,
}

impl FlatIndex {
    /// Create an empty index for `dim`-dimensional vectors.
    #[must_use]
    pub fn new(dim: usize) -> Self {
        Self {
            dim,
            vectors: HashMap::new(),
        }
    }
}

impl EmbeddingIndex for FlatIndex {
    fn insert(&mut self, id: NodeId, vector: Vec<f32>) {
        // In debug builds, assert dimension matches. In release, truncate/pad
        // defensively and log — a DualEmbedder fallback with a different
        // dimension would otherwise produce garbage cosine scores silently.
        if vector.len() != self.dim {
            if cfg!(debug_assertions) {
                panic!(
                    "FlatIndex dimension mismatch: expected {}, got {}",
                    self.dim,
                    vector.len()
                );
            } else {
                tracing::warn!(
                    expected = self.dim,
                    actual = vector.len(),
                    "FlatIndex dimension mismatch — truncating to fit"
                );
            }
        }
        // Defensive: truncate to self.dim so cosine never indexes out of bounds.
        let mut v = vector;
        v.truncate(self.dim);
        self.vectors.insert(id, v);
    }

    fn search(&self, query: &[f32], k: usize) -> Vec<(NodeId, f32)> {
        if k == 0 || self.vectors.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(NodeId, f32)> = self
            .vectors
            .iter()
            .map(|(id, v)| (*id, cosine(query, v)))
            .collect();
        // Partial selection: keep the top-k by descending score.
        let k = k.min(scored.len());
        scored.select_nth_unstable_by(k - 1, |a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        // Sort the chosen k for stable output.
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }

    fn remove(&mut self, id: NodeId) {
        self.vectors.remove(&id);
    }

    fn len(&self) -> usize {
        self.vectors.len()
    }

    fn save(&self, path: &Path) -> Result<(), EmbeddingError> {
        let file = std::fs::File::create(path)?;
        let mut w = BufWriter::new(file);

        w.write_all(MAGIC)?;
        w.write_all(&VERSION.to_le_bytes())?;
        w.write_all(&(self.dim as u32).to_le_bytes())?;
        w.write_all(&(self.vectors.len() as u32).to_le_bytes())?;

        for (id, v) in &self.vectors {
            // NodeId stores index+1 in a NonZeroUsize; persist the inner value.
            w.write_all(&(id.index() as u64).to_le_bytes())?;
            for &f in v {
                w.write_all(&f.to_le_bytes())?;
            }
        }
        w.flush()?;
        Ok(())
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

impl FlatIndex {
    /// Load a flat index from a sidecar written by [`EmbeddingIndex::save`].
    ///
    /// Returns the index and the set of ids it contains (useful for reconciling
    /// against the live graph).
    pub fn load(path: &Path) -> Result<Self, EmbeddingError> {
        let file = std::fs::File::open(path)?;
        let mut r = BufReader::new(file);

        let mut magic = [0u8; 4];
        r.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(EmbeddingError::Api("bad embeddings magic".to_string()));
        }

        let mut buf2 = [0u8; 2];
        r.read_exact(&mut buf2)?;
        let version = u16::from_le_bytes(buf2);
        if version != VERSION {
            return Err(EmbeddingError::Api(format!(
                "unsupported embeddings version: {version}"
            )));
        }

        let mut buf4 = [0u8; 4];
        r.read_exact(&mut buf4)?;
        let dim = u32::from_le_bytes(buf4) as usize;
        r.read_exact(&mut buf4)?;
        let count = u32::from_le_bytes(buf4) as usize;

        let mut vectors = HashMap::with_capacity(count);
        let mut buf8 = [0u8; 8];
        for _ in 0..count {
            r.read_exact(&mut buf8)?;
            let raw_id = u64::from_le_bytes(buf8) as usize;
            // Guard against usize::MAX which would overflow in NodeId::new.
            if raw_id >= isize::MAX as usize {
                return Err(EmbeddingError::Api(format!(
                    "invalid node id in embeddings file: {raw_id}"
                )));
            }
            let id = NodeId::new(raw_id);
            let mut v = Vec::with_capacity(dim);
            for _ in 0..dim {
                r.read_exact(&mut buf4)?;
                v.push(f32::from_le_bytes(buf4));
            }
            vectors.insert(id, v);
        }

        Ok(Self { dim, vectors })
    }
}

/// Cosine similarity for two equal-length f32 slices.
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..n {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom > 0.0 {
        dot / denom
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn v(id: usize, xs: &[f32]) -> (NodeId, Vec<f32>) {
        (NodeId::new(id), xs.to_vec())
    }

    #[test]
    fn search_orders_by_cosine() {
        let mut idx = FlatIndex::new(3);
        idx.insert(v(0, &[1.0, 0.0, 0.0]).0, v(0, &[1.0, 0.0, 0.0]).1);
        idx.insert(v(1, &[0.0, 1.0, 0.0]).0, v(1, &[0.0, 1.0, 0.0]).1);
        idx.insert(v(2, &[0.9, 0.1, 0.0]).0, v(2, &[0.9, 0.1, 0.0]).1);

        let results = idx.search(&[1.0, 0.0, 0.0], 3);
        assert_eq!(results[0].0.index(), 0); // exact match
        // Second should be node 2 (0.9, 0.1) which is closer to (1,0,0) than (0,1,0).
        assert_eq!(results[1].0.index(), 2);
    }

    #[test]
    fn empty_index_search() {
        let idx = FlatIndex::new(4);
        assert!(idx.search(&[1.0, 0.0, 0.0, 0.0], 5).is_empty());
        assert_eq!(idx.len(), 0);
        assert!(idx.is_empty());
    }

    #[test]
    fn k_zero_returns_empty() {
        let mut idx = FlatIndex::new(2);
        idx.insert(v(0, &[1.0, 0.0]).0, v(0, &[1.0, 0.0]).1);
        assert!(idx.search(&[1.0, 0.0], 0).is_empty());
    }

    #[test]
    fn remove_drops_vector() {
        let mut idx = FlatIndex::new(2);
        idx.insert(v(0, &[1.0, 0.0]).0, v(0, &[1.0, 0.0]).1);
        idx.insert(v(1, &[0.0, 1.0]).0, v(1, &[0.0, 1.0]).1);
        idx.remove(v(0, &[1.0, 0.0]).0);
        assert_eq!(idx.len(), 1);
        let r = idx.search(&[1.0, 0.0], 5);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0.index(), 1);
    }

    #[test]
    fn save_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("emb.bin");
        let mut idx = FlatIndex::new(3);
        idx.insert(v(0, &[0.1, 0.2, 0.3]).0, v(0, &[0.1, 0.2, 0.3]).1);
        idx.insert(v(5, &[0.4, 0.5, 0.6]).0, v(5, &[0.4, 0.5, 0.6]).1);
        idx.save(&path).unwrap();

        let loaded = FlatIndex::load(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.dim, 3);
        let r = loaded.search(&[0.4, 0.5, 0.6], 1);
        assert_eq!(r[0].0.index(), 5);
    }

    #[test]
    fn load_rejects_bad_magic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.bin");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"XXXX").unwrap();
        assert!(FlatIndex::load(&path).is_err());
    }

    #[test]
    fn cosine_math() {
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!((cosine(&[1.0, 0.0], &[0.0, 1.0])).abs() < 1e-6);
        assert!((cosine(&[1.0, 1.0], &[1.0, 1.0]) - 1.0).abs() < 1e-6);
        // Zero vector → 0 (not NaN).
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
    }
}
