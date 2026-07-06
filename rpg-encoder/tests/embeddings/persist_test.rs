//! Persistence + concurrency tests for the flat embedding index.

#![cfg(feature = "embeddings")]

use std::sync::Arc;

use rpg_encoder::{EmbeddingIndex, FlatIndex, NodeId};

#[test]
fn save_load_round_trip_preserves_vectors_and_search() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("emb.bin");

    let mut idx = FlatIndex::new(8);
    idx.insert(NodeId::new(0), vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    idx.insert(NodeId::new(1), vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    idx.insert(NodeId::new(5), vec![0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    idx.save(&path).unwrap();

    let loaded = FlatIndex::load(&path).unwrap();
    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded.dimension(), 8);

    // The closest vector to (1,0,...) is node 0.
    let r = loaded.search(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 1);
    assert_eq!(r[0].0, NodeId::new(0));
}

#[test]
fn load_rejects_truncated_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.bin");
    std::fs::write(&path, b"RPGE").unwrap(); // magic only, no version/dim/count
    assert!(FlatIndex::load(&path).is_err());
}

#[test]
fn load_rejects_wrong_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.bin");
    // Valid magic + bad version (999).
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RPGE");
    bytes.extend_from_slice(&999u16.to_le_bytes());
    bytes.extend_from_slice(&8u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    std::fs::write(&path, &bytes).unwrap();
    assert!(FlatIndex::load(&path).is_err());
}

#[test]
fn concurrent_reads_on_shared_index() {
    use std::thread;

    let mut idx = FlatIndex::new(4);
    for i in 0..100 {
        let mut v = vec![0.0f32; 4];
        v[i % 4] = 1.0;
        idx.insert(NodeId::new(i), v);
    }
    let idx = Arc::new(idx);

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let idx = Arc::clone(&idx);
            thread::spawn(move || {
                // Each thread runs searches — reads must not race.
                for i in 0..10 {
                    let mut q = vec![0.0f32; 4];
                    q[i % 4] = 1.0;
                    let r = idx.search(&q, 5);
                    assert_eq!(r.len(), 5);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}
