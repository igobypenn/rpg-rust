//! Benchmark: FlatIndex vs ZvecIndex on synthetic data.
//!
//! Generates N random vectors, inserts them into both backends, and measures
//! p50/p99 search latency + recall@10 (using FlatIndex as ground truth).
//!
//! Run with:
//! ```sh
//! cargo run --release --example bench_embeddings --features zvec -- --n 5000 --dim 128 --queries 100
//! ```
//!
//! Without the `zvec` feature, only the FlatIndex benchmark runs.

use std::time::Instant;

#[cfg(feature = "embeddings")]
use rpg_encoder::{EmbeddingIndex, FlatIndex, NodeId};
#[cfg(feature = "zvec")]
use rpg_encoder::{ZvecIndex, ZvecIndexKind};

/// Deterministic pseudo-random vector generator (no rand dep needed).
fn random_vector(seed: usize, dim: usize) -> Vec<f32> {
    (0..dim)
        .map(|i| {
            let x = (seed.wrapping_mul(31).wrapping_add(i.wrapping_mul(17)))
                .wrapping_mul(0x9E3779B1);
            (x as f32 / u32::MAX as f32).abs()
        })
        .collect()
}

#[cfg(feature = "embeddings")]
fn run() {
    let args: Vec<String> = std::env::args().collect();
    let mut n = 5000usize;
    let mut dim = 128usize;
    let mut queries = 100usize;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--n" if i + 1 < args.len() => {
                n = args[i + 1].parse().unwrap_or(n);
                i += 2;
            }
            "--dim" if i + 1 < args.len() => {
                dim = args[i + 1].parse().unwrap_or(dim);
                i += 2;
            }
            "--queries" if i + 1 < args.len() => {
                queries = args[i + 1].parse().unwrap_or(queries);
                i += 2;
            }
            _ => i += 1,
        }
    }

    println!("=== Embedding Index Benchmark ===");
    println!("Config: {n} vectors, {dim} dims, {queries} queries\n");

    // Generate data.
    let vectors: Vec<Vec<f32>> = (0..n).map(|i| random_vector(i, dim)).collect();
    let query_vecs: Vec<Vec<f32>> = (0..queries).map(|i| random_vector(i + n, dim)).collect();

    // --- FlatIndex ---
    let dir = tempfile::tempdir().unwrap();
    let flat_path = dir.path().join("flat.bin");
    let t0 = Instant::now();
    let mut flat = FlatIndex::new(dim);
    for (i, v) in vectors.iter().enumerate() {
        flat.insert(NodeId::new(i), v.clone());
    }
    let flat_insert = t0.elapsed();

    let t0 = Instant::now();
    let flat_results: Vec<Vec<(NodeId, f32)>> = query_vecs
        .iter()
        .map(|q| flat.search(q, 10))
        .collect();
    let flat_search = t0.elapsed();

    let flat_latencies: Vec<u128> = query_vecs
        .iter()
        .map(|q| {
            let t = Instant::now();
            let _ = flat.search(q, 10);
            t.elapsed().as_micros()
        })
        .collect();
    let mut flat_sorted = flat_latencies.clone();
    flat_sorted.sort();

    println!("--- FlatIndex ---");
    println!("Insert: {:?}", flat_insert);
    println!("Search ({} queries): {:?}", queries, flat_search);
    println!(
        "p50: {}μs, p99: {}μs\n",
        flat_sorted[flat_sorted.len() / 2],
        flat_sorted[flat_sorted.len() * 99 / 100]
    );

    flat.save(&flat_path).unwrap();
    let flat_size = std::fs::metadata(&flat_path).map(|m| m.len()).unwrap_or(0);
    println!("Sidecar size: {} bytes ({:.1} KB)\n", flat_size, flat_size as f64 / 1024.0);

    #[cfg(feature = "zvec")]
    {
        // --- ZvecIndex (flat) ---
        let zdir = tempfile::tempdir().unwrap();
        let zpath = zdir.path().join("zflat");
        let t0 = Instant::now();
        let mut zflat = ZvecIndex::with_kind(&zpath, dim, ZvecIndexKind::Flat).unwrap();
        for (i, v) in vectors.iter().enumerate() {
            zflat.insert(NodeId::new(i), v.clone());
        }
        let zflat_insert = t0.elapsed();

        // Warm up (first query may be slower due to index building).
        let _ = zflat.search(&query_vecs[0], 10);

        let t0 = Instant::now();
        let zflat_results: Vec<Vec<(NodeId, f32)>> = query_vecs
            .iter()
            .map(|q| zflat.search(q, 10))
            .collect();
        let zflat_search = t0.elapsed();

        let zflat_latencies: Vec<u128> = query_vecs
            .iter()
            .map(|q| {
                let t = Instant::now();
                let _ = zflat.search(q, 10);
                t.elapsed().as_micros()
            })
            .collect();
        let mut zflat_sorted = zflat_latencies.clone();
        zflat_sorted.sort();

        println!("--- ZvecIndex (flat) ---");
        println!("Insert: {:?}", zflat_insert);
        println!("Search ({} queries): {:?}", queries, zflat_search);
        println!(
            "p50: {}μs, p99: {}μs\n",
            zflat_sorted[zflat_sorted.len() / 2],
            zflat_sorted[zflat_sorted.len() * 99 / 100]
        );

        // --- Recall@10: how many of FlatIndex's top-10 does ZvecIndex find? ---
        let mut recall_sum = 0.0f64;
        for (fr, zr) in flat_results.iter().zip(zflat_results.iter()) {
            let flat_ids: std::collections::HashSet<NodeId> = fr.iter().map(|(id, _)| *id).collect();
            let z_ids: std::collections::HashSet<NodeId> = zr.iter().map(|(id, _)| *id).collect();
            let overlap = flat_ids.intersection(&z_ids).count();
            recall_sum += overlap as f64 / flat_ids.len() as f64;
        }
        let recall = recall_sum / queries as f64;
        println!("Recall@10 (Zvec vs Flat ground truth): {:.1}%\n", recall * 100.0);

        // --- ZvecIndex (HNSW) — only if scale justifies it ---
        if n >= 1000 {
            let hpath = zdir.path().join("hnsw");
            let t0 = Instant::now();
            let mut zhnsw =
                ZvecIndex::with_kind(&hpath, dim, ZvecIndexKind::Hnsw { m: 16, ef_construction: 200 }).unwrap();
            for (i, v) in vectors.iter().enumerate() {
                zhnsw.insert(NodeId::new(i), v.clone());
            }
            let zhnsw_insert = t0.elapsed();

            let _ = zhnsw.search(&query_vecs[0], 10); // warm up

            let t0 = Instant::now();
            let _zresults: Vec<_> = query_vecs.iter().map(|q| zhnsw.search(q, 10)).collect();
            let zhnsw_search = t0.elapsed();

            println!("--- ZvecIndex (HNSW m=16, ef=200) ---");
            println!("Insert: {:?}", zhnsw_insert);
            println!("Search ({} queries): {:?}", queries, zhnsw_search);
        }
    }

    #[cfg(not(feature = "zvec"))]
    {
        println!("(zvec feature not enabled — run with --features zvec for the comparison)");
    }
}

fn main() {
    #[cfg(feature = "embeddings")]
    run();

    #[cfg(not(feature = "embeddings"))]
    {
        eprintln!("This example requires the `embeddings` feature.");
        eprintln!("Run with: cargo run --release --example bench_embeddings --features embeddings");
    }
}
