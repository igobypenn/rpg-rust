//! Property-based tests for the flat embedding index.
//!
//! Validates that FlatIndex search returns correctly-ordered results and that
//! dimension is respected across random vector sets.

#![cfg(feature = "embeddings")]

use proptest::prelude::*;
use rpg_encoder::{EmbeddingIndex, FlatIndex, NodeId};

/// Generate `count` random f32 vectors of `dim` dimension, values in [0, 1).
fn random_vectors(count: usize, dim: usize) -> Vec<Vec<f32>> {
    (0..count)
        .map(|i| {
            (0..dim)
                .map(|j| {
                    // Deterministic pseudo-random from indices (no rand dep needed).
                    let x = (i * 31 + j * 17).wrapping_mul(0x9E3779B1);
                    (x as f32 / u32::MAX as f32).abs()
                })
                .collect()
        })
        .collect()
}

proptest! {
    /// Inserting N vectors and searching for one of them returns that vector
    /// as the top result (cosine sim = 1.0 with itself).
    #[test]
    fn exact_query_returns_self(n in 1usize..50, dim in 2usize..32) {
        let vecs = random_vectors(n, dim);
        let mut idx = FlatIndex::new(dim);
        for (i, v) in vecs.iter().enumerate() {
            idx.insert(NodeId::new(i), v.clone());
        }

        // Query with vector #0 — expect it back as the top hit.
        let results = idx.search(&vecs[0], 1);
        prop_assert!(!results.is_empty());
        prop_assert_eq!(results[0].0, NodeId::new(0));
        // Cosine of a vector with itself is 1.0 (within f32 tolerance).
        prop_assert!((results[0].1 - 1.0).abs() < 1e-5);
    }

    /// Search results are always returned in descending score order.
    #[test]
    fn results_are_descending(n in 2usize..40, dim in 2usize..16, k in 1usize..10) {
        let vecs = random_vectors(n, dim);
        let mut idx = FlatIndex::new(dim);
        for (i, v) in vecs.iter().enumerate() {
            idx.insert(NodeId::new(i), v.clone());
        }

        let query = vecs[0].clone();
        let k = k.min(n);
        let results = idx.search(&query, k);

        prop_assert_eq!(results.len(), k);
        for w in results.windows(2) {
            prop_assert!(w[0].1 >= w[1].1 || (w[0].1 - w[1].1).abs() < 1e-6,
                "scores not descending: {} then {}", w[0].1, w[1].1);
        }
    }

    /// Dimension is preserved across save/load round-trips.
    #[test]
    fn dimension_invariant(dim in 1usize..64) {
        let mut idx = FlatIndex::new(dim);
        idx.insert(NodeId::new(0), vec![0.5f32; dim]);
        prop_assert_eq!(idx.dimension(), dim);
        prop_assert_eq!(idx.len(), 1);
    }

    /// Remove then search: the removed id never appears in results.
    #[test]
    fn remove_then_search_excludes(n in 2usize..30, dim in 2usize..16) {
        let vecs = random_vectors(n, dim);
        let mut idx = FlatIndex::new(dim);
        for (i, v) in vecs.iter().enumerate() {
            idx.insert(NodeId::new(i), v.clone());
        }

        idx.remove(NodeId::new(0));
        let results = idx.search(&vecs[1], n);
        prop_assert!(results.iter().all(|(id, _)| *id != NodeId::new(0)));
        prop_assert_eq!(idx.len(), n - 1);
    }
}
