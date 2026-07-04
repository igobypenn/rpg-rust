//! Benchmarks for the semantic-encoding CPU pipeline.
//!
//! Covers the functional-abstraction phases and the `find_node_in_file` lookup
//! that dominates the enrichment loop. No network calls — these measure the
//! pure local work that runs after LLM feature extraction.
//!
//! Run: `cargo bench --bench semantic`

use std::hint::black_box;
use std::path::PathBuf;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use rpg_encoder::core::{EdgeType, Node, NodeCategory, NodeId, NodeLevel, RpgGraph};
use rpg_encoder::encoder::FunctionalAbstraction;

/// Build a graph of `count` V^L (function) nodes spread across `files` files,
/// each carrying a semantic feature. Edges: a Contains edge per node to its
/// (synthetic) file node so the graph is realistic.
fn build_graph_with_semantics(count: usize, files: usize) -> RpgGraph {
    let mut graph = RpgGraph::new();

    // One file node per file; functions distributed round-robin across files.
    let mut file_ids = Vec::with_capacity(files);
    for f in 0..files {
        let path = PathBuf::from(format!("src/mod_{:04}/file.rs", f));
        let fid = graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::File,
                "file",
                "rust",
                format!("file_{f}"),
            )
            .with_path(path),
        );
        file_ids.push(fid);
    }

    for i in 0..count {
        let path = PathBuf::from(format!("src/mod_{:04}/file.rs", i % files));
        let node = Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "function",
            "rust",
            format!("func_{i}"),
        )
        .with_path(path)
        .with_node_level(NodeLevel::Low)
        .with_semantic_feature(format!("handles operation {} for module", i % 32));
        let nid = graph.add_node(node);
        graph.add_typed_edge(file_ids[i % files], nid, EdgeType::Contains);
    }

    graph
}

/// Benchmark the four functional-abstraction phases plus the full `run()`.
fn functional_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("functional");
    group.throughput(Throughput::Elements(1));

    for &size in &[100usize, 500, 2000] {
        let files = (size / 10).max(1);
        let graph = build_graph_with_semantics(size, files);
        let label = format!("{size}_nodes");

        // Phase 2.1: collect — read-only over the graph.
        group.bench_with_input(BenchmarkId::new("collect_semantic_features", &label), &(), |b, _| {
            b.iter_batched(
                || graph.clone(),
                |mut g| {
                    let fa = FunctionalAbstraction::new(&mut g);
                    black_box(fa.collect_semantic_features())
                },
                criterion::BatchSize::SmallInput,
            )
        });

        // Phase 2.2: induce centroids from collected features.
        let features = {
            let mut g = graph.clone();
            let fa = FunctionalAbstraction::new(&mut g);
            fa.collect_semantic_features()
        };
        group.bench_with_input(BenchmarkId::new("induce_centroids_heuristic", &label), &(), |b, _| {
            b.iter_batched(
                || graph.clone(),
                |mut g| {
                    let fa = FunctionalAbstraction::new(&mut g);
                    black_box(fa.induce_centroids_heuristic(black_box(&features)))
                },
                criterion::BatchSize::SmallInput,
            )
        });

        // Phase 2.4: aggregate (the O(features × centroids) path). Cloned per
        // iter because it mutates the graph.
        group.bench_with_input(BenchmarkId::new("aggregate_hierarchy", &label), &(), |b, _| {
            b.iter_batched(
                || {
                    let mut g = graph.clone();
                    let mut fa = FunctionalAbstraction::new(&mut g);
                    let centroids = fa.induce_centroids_heuristic(&features);
                    let centroid_map = fa.create_centroid_nodes(&centroids);
                    (g, centroid_map)
                },
                |(mut g, centroid_map)| {
                    let mut fa = FunctionalAbstraction::new(&mut g);
                    fa.aggregate_hierarchy(black_box(&features), black_box(&centroid_map))
                        .unwrap()
                },
                criterion::BatchSize::SmallInput,
            )
        });

        // Full pipeline run() (mutates graph — clone per iter).
        group.bench_with_input(BenchmarkId::new("run_full", &label), &(), |b, _| {
            b.iter_batched(
                || graph.clone(),
                |mut g| {
                    let mut fa = FunctionalAbstraction::new(&mut g);
                    fa.run().unwrap()
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

/// Benchmark `find_node_in_file` — the O(n) per-entity scan in the
/// enrichment loop. At scale this dominates because it's called once per
/// extracted entity.
fn find_node_in_file(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/find_node_in_file");
    group.throughput(Throughput::Elements(1));

    for &size in &[500usize, 2000, 5000] {
        let files = (size / 10).max(1);
        let graph = build_graph_with_semantics(size, files);

        // A name that exists roughly in the middle of the node list, and a
        // path that matches one of its files.
        let mid = size / 2;
        let target_path = PathBuf::from(format!("src/mod_{:04}/file.rs", mid % files));
        let target_name = format!("func_{mid}");

        // Worst case: a name that does NOT exist (full scan, no short-circuit).
        let missing_name = "does_not_exist";

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("hit", size),
            &(target_path.clone(), target_name.clone()),
            |b, (path, name)| {
                b.iter(|| black_box(graph.find_node_in_file(black_box(path), black_box(name))))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("miss", size),
            &(target_path.clone(), missing_name.to_string()),
            |b, (path, name)| {
                b.iter(|| black_box(graph.find_node_in_file(black_box(path), black_box(name))))
            },
        );
    }

    group.finish();
}

criterion_group!(semantic_benches, functional_pipeline, find_node_in_file);
criterion_main!(semantic_benches);
