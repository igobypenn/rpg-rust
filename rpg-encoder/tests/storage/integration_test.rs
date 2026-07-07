//! Critical-path integration tests: encode → persist → load → query.
//!
//! These tests verify the full round-trip that rpg-mcp relies on for the
//! committed-graph workflow. The prior version only checked node_count; this
//! version asserts edge counts, node fields, queryability, and NodeId stability.

use rpg_encoder::{EdgeType, NodeCategory, NodeId, RpgEncoder, RpgSnapshot, RpgStore};
use tempfile::TempDir;

/// A richer fixture than `fn main() {}` — has imports, structs, impls, calls.
const FIXTURE: &str = r#"
use std::io::Read;

pub struct Config {
    pub path: String,
}

impl Config {
    pub fn new(path: &str) -> Self {
        Config { path: path.to_string() }
    }

    pub fn load(&self) -> String {
        self.path.clone()
    }
}

pub fn main() {
    let config = Config::new("input.txt");
    let data = config.load();
    println!("{}", data);
}
"#;

fn create_test_repo(dir: &std::path::Path) {
    let src = dir.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("main.rs"), FIXTURE).unwrap();
}

#[test]
fn test_init_and_open_store() {
    let dir = TempDir::new().unwrap();
    create_test_repo(dir.path());

    let mut encoder = RpgEncoder::new().unwrap();
    let store = encoder.init_store(dir.path()).unwrap();
    assert_eq!(store.patch_count(), 0);

    let mut encoder2 = RpgEncoder::new().unwrap();
    let store2 = encoder2.open_store(dir.path()).unwrap();
    assert_eq!(store2.patch_count(), 0);
}

#[test]
fn test_encode_save_load_cycle() {
    let dir = TempDir::new().unwrap();
    create_test_repo(dir.path());
    let repo = dir.path().join("src");

    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(&repo).unwrap();
    assert!(result.graph.node_count() > 0);

    encoder.init_store(dir.path()).unwrap();

    let mut snapshot = RpgSnapshot::new("test", dir.path());
    snapshot.graph = result.graph.clone();
    snapshot.compute_file_hashes().ok();

    encoder.store_mut().unwrap().save_base(&snapshot).unwrap();

    let loaded = RpgStore::open(dir.path()).unwrap().load().unwrap();

    // 1. Node count round-trips.
    assert_eq!(
        loaded.graph.node_count(),
        snapshot.graph.node_count(),
        "node count must match after save/load"
    );

    // 2. Edge count round-trips.
    assert_eq!(
        loaded.graph.edge_count(),
        snapshot.graph.edge_count(),
        "edge count must match after save/load"
    );

    // 3. Node fields survive — pick a Function node and verify its fields.
    let func = snapshot
        .graph
        .nodes()
        .find(|n| n.category == NodeCategory::Function && n.name == "main")
        .expect("fixture has a 'main' function");
    let loaded_func = loaded
        .graph
        .get_node(func.id)
        .expect("function node must be accessible by id after reload");
    assert_eq!(loaded_func.name, "main");
    assert_eq!(loaded_func.kind, "fn");
    assert_eq!(loaded_func.language, "rust");
    assert_eq!(loaded_func.category, NodeCategory::Function);
    assert_eq!(
        loaded_func.path.as_deref(),
        func.path.as_deref(),
        "path must survive round-trip"
    );

    // 4. Queryability — get_node works, find_node_by_path works, edges work.
    assert!(loaded.graph.get_node(func.id).is_some(), "get_node must work");
    assert!(
        loaded.graph.find_node_by_path(std::path::Path::new("main.rs")).is_some(),
        "find_node_by_path must work after reload"
    );

    // 5. Contains edges survive — the file node should have children.
    let file_node = loaded
        .graph
        .nodes()
        .find(|n| n.category == NodeCategory::File)
        .expect("file node exists");
    let children = loaded.graph.edges_from(file_node.id);
    assert!(
        children.iter().any(|(_, e)| e.edge_type == EdgeType::Contains),
        "Contains edges must survive round-trip"
    );

    // 6. NodeId stability — the same node has the same id before and after.
    assert_eq!(
        loaded_func.id, func.id,
        "NodeId must be stable across save/reload (critical for embeddings)"
    );
}

#[test]
fn test_loaded_graph_supports_queries() {
    let dir = TempDir::new().unwrap();
    create_test_repo(dir.path());
    let repo = dir.path().join("src");

    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(&repo).unwrap();

    encoder.init_store(dir.path()).unwrap();
    let mut snapshot = RpgSnapshot::new("test", dir.path());
    snapshot.graph = result.graph;
    snapshot.compute_file_hashes().ok();
    encoder.store_mut().unwrap().save_base(&snapshot).unwrap();

    let loaded = RpgStore::open(dir.path()).unwrap().load().unwrap();

    // The loaded graph must support the queries that MCP tools rely on.
    assert!(loaded.graph.node_count() > 3, "should have repo + file + functions");

    // find_node_by_name works.
    let config = loaded.graph.find_node_by_name("Config", None);
    assert!(config.is_some(), "find_node_by_name must work after reload");

    // in_degree works (non-allocating).
    let func = loaded
        .graph
        .nodes()
        .find(|n| n.name == "main")
        .expect("main exists");
    assert!(
        loaded.graph.in_degree(func.id) >= 0,
        "in_degree must not panic after reload"
    );

    // bfs_reachable works (the canonical BFS).
    let reachable = loaded.graph.bfs_reachable(func.id, 2, false, None);
    assert!(
        !reachable.is_empty(),
        "bfs_reachable must return at least the start node"
    );
}
