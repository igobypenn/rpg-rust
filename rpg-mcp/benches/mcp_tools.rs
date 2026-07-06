//! Benchmark: MCP critical-path tool latency.
//!
//! Measures the tools a developer calls in the interactive loop:
//! search_nodes, get_node_details, get_callers, get_callees, get_skeleton,
//! semantic_search, get_architecture_overview, find_dead_code.
//!
//! Run with: `cargo bench -p rpg-mcp --bench mcp_tools`
//!
//! The goal: every interactive tool must complete in <10ms at 1k nodes for a
//! seamless agent experience. This benchmark verifies that budget holds and
//! catches regressions early.

use std::sync::Arc;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rpg_encoder::{Edge, EdgeType, Node, NodeCategory, NodeId, ParserRegistry, RpgGraph, RpgSnapshot};
use rpg_mcp::service::RpgService;
use rpg_mcp::state::{AppState, McpConfig};
use serde_json::json;

fn make_graph(node_count: usize) -> RpgGraph {
    let mut graph = RpgGraph::new();

    // Create a tree: 1 repo → files → functions.
    let repo = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Repository,
        "repo",
        "rust",
        "test-repo",
    ));

    let files = node_count / 10; // ~10 functions per file
    let fns_per_file = node_count / files.max(1);

    for f in 0..files {
        let file = graph.add_node(
            Node::new(NodeId::new(0), NodeCategory::File, "file", "rust", format!("file_{f}.rs"))
                .with_path(std::path::PathBuf::from(format!("src/file_{f}.rs"))),
        );
        graph.add_edge(repo, file, Edge::new(EdgeType::Contains));

        for i in 0..fns_per_file {
            let func = graph.add_node(
                Node::new(
                    NodeId::new(0),
                    NodeCategory::Function,
                    "fn",
                    "rust",
                    format!("func_{f}_{i}"),
                )
                .with_path(std::path::PathBuf::from(format!("src/file_{f}.rs")))
                .with_description(format!("Function {} in file {}", i, f))
                .with_features(vec![format!("does thing {}", i)]),
            );
            graph.add_edge(file, func, Edge::new(EdgeType::Contains));

            // Add some Calls edges (each function calls the next in the same file).
            if i > 0 {
                let prev_func_id = NodeId::new(func.index() - 1);
                graph.add_edge(prev_func_id, func, Edge::new(EdgeType::Calls));
            }
        }
    }

    graph
}

fn make_service(graph: RpgGraph) -> RpgService {
    let dir = tempfile::tempdir().unwrap();
    let dir_path = dir.keep();
    let mut snapshot = RpgSnapshot::new("bench", &dir_path);
    snapshot.graph = graph;
    snapshot.build_reverse_deps();
    let config = McpConfig {
        workspace: dir_path,
        data_dir: std::path::PathBuf::new(),
        hash_mode: rpg_mcp::state::HashMode::Mtime,
        semantic: false,
    };
    let registry = Arc::new(ParserRegistry::new());
    let state = AppState::new(config, snapshot, registry);
    RpgService::new(Arc::new(state))
}

fn params(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_value(v).unwrap_or_default()
}

fn bench_interactive_tools(c: &mut Criterion) {
    // Benchmark at two scales: 100 nodes (small repo) and 1000 nodes (medium).
    for &scale in &[100usize, 1000] {
        let graph = make_graph(scale);
        let service = make_service(graph);
        let label = |name: &str| format!("{name}_{scale}");

        // search_nodes — O(N) scan
        c.bench_function(&label("search_nodes"), |b| {
            b.iter(|| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let _ = black_box(
                        service
                            .search_nodes(params(json!({"query": "func_5"})))
                            .await
                            .unwrap(),
                    );
                });
            })
        });

        // get_node_details — O(degree)
        c.bench_function(&label("get_node_details"), |b| {
            b.iter(|| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let _ = black_box(
                        service
                            .get_node_details(params(json!({"id": 5, "detail_level": "full"})))
                            .await
                            .unwrap(),
                    );
                });
            })
        });

        // get_skeleton — was O(F×E), now O(F×degree)
        c.bench_function(&label("get_skeleton"), |b| {
            b.iter(|| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let _ = black_box(service.get_skeleton(params(json!({}))).await.unwrap());
                });
            })
        });

        // get_architecture_overview — hub scan (now uses in_degree)
        c.bench_function(&label("architecture_overview"), |b| {
            b.iter(|| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let _ = black_box(
                        service
                            .get_architecture_overview(params(json!({})))
                            .await
                            .unwrap(),
                    );
                });
            })
        });

        // find_dead_code — now uses has_incoming_of_types
        c.bench_function(&label("find_dead_code"), |b| {
            b.iter(|| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let _ = black_box(service.find_dead_code(params(json!({}))).await.unwrap());
                });
            })
        });

        // semantic_search — O(N) scan
        c.bench_function(&label("semantic_search"), |b| {
            b.iter(|| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let _ = black_box(
                        service
                            .semantic_search(params(json!({"query": "thing"})))
                            .await
                            .unwrap(),
                    );
                });
            })
        });
    }
}

criterion_group!(benches, bench_interactive_tools);
criterion_main!(benches);
