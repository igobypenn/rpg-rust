mod fixtures;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rpg_encoder::{Edge, EdgeType, Node, NodeCategory, NodeId, RpgGraph};
use std::path::PathBuf;

fn graph_insert_node(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/insert_node");

    group.bench_function("single_node", |b| {
        b.iter(|| {
            let mut graph = RpgGraph::new();
            let node = Node::new(
                NodeId::new(0),
                NodeCategory::Function,
                "function",
                "rust",
                "test_func",
            );
            graph.add_node(black_box(node))
        })
    });

    for size in [10usize, 100, 1000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("batch", size), &size, |b, &size| {
            b.iter_batched(
                || RpgGraph::new(),
                |mut graph| {
                    for i in 0..size {
                        let node = Node::new(
                            NodeId::new(i),
                            NodeCategory::Function,
                            "function",
                            "rust",
                            &format!("func_{}", i),
                        );
                        graph.add_node(node);
                    }
                    black_box(graph)
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn graph_insert_edge(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/insert_edge");

    let setup_graph = |size: usize| {
        let mut graph = RpgGraph::new();
        let mut ids = Vec::with_capacity(size);
        for i in 0..size {
            let node = Node::new(
                NodeId::new(i),
                NodeCategory::Function,
                "function",
                "rust",
                &format!("func_{}", i),
            );
            let id = graph.add_node(node);
            ids.push(id);
        }
        (graph, ids)
    };

    group.bench_function("single_edge", |b| {
        b.iter_batched(
            || {
                let (graph, ids) = setup_graph(2);
                (graph, ids[0], ids[1])
            },
            |(mut graph, src, tgt)| {
                graph.add_edge(black_box(src), black_box(tgt), Edge::new(EdgeType::Calls))
            },
            criterion::BatchSize::SmallInput,
        )
    });

    for size in [10usize, 100, 500] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_sequential", size),
            &size,
            |b, &size| {
                b.iter_batched(
                    || setup_graph(size + 1),
                    |(mut graph, ids)| {
                        for i in 0..size {
                            graph.add_edge(ids[i], ids[i + 1], Edge::new(EdgeType::Calls));
                        }
                        black_box(graph)
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    for size in [10usize, 100, 500] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("batch_random", size), &size, |b, &size| {
            b.iter_batched(
                || setup_graph(size),
                |(mut graph, ids)| {
                    for i in 0..size {
                        let src = ids[i % ids.len()];
                        let tgt = ids[(i + 1) % ids.len()];
                        graph.add_edge(src, tgt, Edge::new(EdgeType::Calls));
                    }
                    black_box(graph)
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn create_populated_graph(node_count: usize, edge_density: f32) -> RpgGraph {
    let mut graph = RpgGraph::new();
    let mut ids = Vec::with_capacity(node_count);

    for i in 0..node_count {
        let node = Node::new(
            NodeId::new(i),
            NodeCategory::Function,
            "function",
            "rust",
            &format!("func_{}", i),
        )
        .with_path(PathBuf::from(format!("src/module_{:04}.rs", i)));
        let id = graph.add_node(node);
        ids.push(id);
    }

    let edge_count = (node_count as f32 * edge_density) as usize;
    for i in 0..edge_count {
        let src = ids[i % node_count];
        let tgt = ids[(i + 1) % node_count];
        graph.add_edge(src, tgt, Edge::new(EdgeType::Calls));
    }

    graph
}

fn graph_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/lookup");

    let graph_100 = create_populated_graph(100, 2.0);
    let graph_1000 = create_populated_graph(1000, 2.0);

    group.bench_function("get_node_by_id/small", |b| {
        b.iter(|| {
            for i in 0..100 {
                black_box(graph_100.get_node(NodeId::new(i)));
            }
        })
    });

    group.bench_function("get_node_by_id/large", |b| {
        b.iter(|| {
            for i in 0..1000 {
                black_box(graph_1000.get_node(NodeId::new(i)));
            }
        })
    });

    group.bench_function("find_node_by_name/small", |b| {
        b.iter(|| {
            for i in 0..100 {
                black_box(graph_100.find_node_by_name(&format!("func_{}", i), None));
            }
        })
    });

    group.bench_function("find_node_by_name/large", |b| {
        b.iter(|| {
            for i in 0..1000 {
                black_box(graph_1000.find_node_by_name(&format!("func_{}", i), None));
            }
        })
    });

    let path = PathBuf::from("src/module_0042.rs");
    group.bench_function("find_node_by_path", |b| {
        b.iter(|| black_box(graph_1000.find_node_by_path(&path)))
    });

    group.finish();
}

fn graph_traversal(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/traversal");

    group.bench_function("nodes_iterator/100", |b| {
        let graph = create_populated_graph(100, 0.0);
        b.iter(|| {
            let count = graph.nodes().count();
            black_box(count)
        })
    });

    group.bench_function("nodes_iterator/1000", |b| {
        let graph = create_populated_graph(1000, 0.0);
        b.iter(|| {
            let count = graph.nodes().count();
            black_box(count)
        })
    });

    group.bench_function("edges_iterator/100", |b| {
        let graph = create_populated_graph(100, 2.0);
        b.iter(|| {
            let count = graph.edges().count();
            black_box(count)
        })
    });

    group.bench_function("edges_iterator/1000", |b| {
        let graph = create_populated_graph(1000, 2.0);
        b.iter(|| {
            let count = graph.edges().count();
            black_box(count)
        })
    });

    let graph = create_populated_graph(100, 3.0);
    group.bench_function("children_of", |b| {
        b.iter(|| {
            for i in 0..10 {
                black_box(graph.children_of(NodeId::new(i)));
            }
        })
    });

    let graph = create_populated_graph(100, 5.0);
    group.bench_function("neighbors", |b| {
        b.iter(|| {
            for i in 0..10 {
                black_box(graph.neighbors(NodeId::new(i)));
            }
        })
    });

    group.bench_function("predecessors", |b| {
        b.iter(|| {
            for i in 0..10 {
                black_box(graph.predecessors(NodeId::new(i)));
            }
        })
    });

    group.bench_function("successors", |b| {
        b.iter(|| {
            for i in 0..10 {
                black_box(graph.successors(NodeId::new(i)));
            }
        })
    });

    group.finish();
}

fn graph_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/serialization");

    let graph_small = create_populated_graph(100, 2.0);
    let graph_medium = create_populated_graph(500, 2.0);
    let graph_large = create_populated_graph(1000, 2.0);

    group.throughput(Throughput::Elements(100));
    group.bench_function("to_json_compact/100_nodes", |b| {
        b.iter(|| rpg_encoder::to_json_compact(black_box(&graph_small)))
    });

    group.throughput(Throughput::Elements(500));
    group.bench_function("to_json_compact/500_nodes", |b| {
        b.iter(|| rpg_encoder::to_json_compact(black_box(&graph_medium)))
    });

    group.throughput(Throughput::Elements(1000));
    group.bench_function("to_json_compact/1000_nodes", |b| {
        b.iter(|| rpg_encoder::to_json_compact(black_box(&graph_large)))
    });

    let json_small = serde_json::to_string(&graph_small).unwrap();
    group.throughput(Throughput::Elements(100));
    group.bench_function("deserialize/100_nodes", |b| {
        b.iter(|| {
            let graph: RpgGraph = serde_json::from_str(black_box(&json_small)).unwrap();
            black_box(graph)
        })
    });

    group.finish();
}

fn graph_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/operations");

    group.bench_function("node_count", |b| {
        let graph = create_populated_graph(1000, 2.0);
        b.iter(|| black_box(graph.node_count()))
    });

    group.bench_function("edge_count", |b| {
        let graph = create_populated_graph(1000, 2.0);
        b.iter(|| black_box(graph.edge_count()))
    });

    group.bench_function("edges_from/10_nodes", |b| {
        let graph = create_populated_graph(100, 5.0);
        b.iter(|| {
            for i in 0..10 {
                black_box(graph.edges_from(NodeId::new(i)));
            }
        })
    });

    group.bench_function("edges_to/10_nodes", |b| {
        let graph = create_populated_graph(100, 5.0);
        b.iter(|| {
            for i in 0..10 {
                black_box(graph.edges_to(NodeId::new(i)));
            }
        })
    });

    group.bench_function("edge_between", |b| {
        let graph = create_populated_graph(100, 2.0);
        b.iter(|| {
            for i in 0..50 {
                black_box(graph.edge_between(NodeId::new(i), NodeId::new(i + 1)));
            }
        })
    });

    group.finish();
}

fn graph_mutation(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph/mutation");

    group.bench_function("remove_node", |b| {
        b.iter_batched(
            || create_populated_graph(100, 2.0),
            |mut graph| {
                for i in 0..10 {
                    black_box(graph.remove_node(NodeId::new(i)));
                }
                graph
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.bench_function("remove_file_nodes", |b| {
        b.iter_batched(
            || {
                let mut graph = RpgGraph::new();
                for i in 0..100 {
                    let path = PathBuf::from("src/test.rs");
                    let node = Node::new(
                        NodeId::new(i),
                        NodeCategory::Function,
                        "function",
                        "rust",
                        &format!("func_{}", i),
                    )
                    .with_path(path.clone());
                    graph.add_node(node);
                }
                graph
            },
            |mut graph| {
                let path = PathBuf::from("src/test.rs");
                black_box(graph.remove_file_nodes(&path));
                graph
            },
            criterion::BatchSize::SmallInput,
        )
    });

    let graph = create_populated_graph(100, 2.0);
    let ids: Vec<NodeId> = graph.nodes().take(10).map(|n| n.id).collect();

    group.bench_function("remove_edges_for_nodes", |b| {
        b.iter_batched(
            || graph.clone(),
            |mut graph| {
                black_box(graph.remove_edges_for_nodes(&ids));
                graph
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

criterion_group!(
    graph_benches,
    graph_insert_node,
    graph_insert_edge,
    graph_lookup,
    graph_traversal,
    graph_serialization,
    graph_operations,
    graph_mutation,
);

criterion_main!(graph_benches);
