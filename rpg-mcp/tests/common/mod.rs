//! Shared test helpers: encode the basic.rs fixture into a graph and provide
//! BFS/node-lookup utilities that mirror what the MCP tools do.
//!
//! Helpers here are used across multiple test targets; each target compiles
//! this module independently, so unused-in-one-target warnings are expected.

#![allow(dead_code)]

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use rpg_encoder::{EdgeType, NodeId, ParserRegistry, RpgEncoder, RpgGraph, RpgSnapshot};
use rpg_mcp::service::RpgService;
use rpg_mcp::state::{AppState, McpConfig};

/// Encode the basic.rs fixture into a graph. Returns (graph, repo_dir).
pub fn encoded_fixture_with_dir() -> (RpgGraph, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.keep();

    // Copy the fixture into the temp dir so the encoder can walk it.
    let fixture = include_str!("../../../rpg-encoder/tests/fixtures/rust/basic.rs");
    std::fs::create_dir_all(dir_path.join("src")).expect("mkdir src");
    std::fs::write(dir_path.join("src/main.rs"), fixture).expect("write fixture");

    let mut encoder = RpgEncoder::new().expect("encoder");
    let result = encoder.encode(&dir_path).expect("encode");
    (result.graph, dir_path)
}

/// Encode the fixture and return just the graph.
pub fn encoded_fixture_graph() -> RpgGraph {
    encoded_fixture_with_dir().0
}

/// Encode the scenario fixture (rich relationships: traits, impls, calls,
/// dead code, FFI). Returns (graph, repo_dir).
pub fn encoded_scenario_with_dir() -> (RpgGraph, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.keep();

    let fixture = include_str!("../fixtures/scenario_lib.rs");
    std::fs::create_dir_all(dir_path.join("src")).expect("mkdir src");
    std::fs::write(dir_path.join("src/lib.rs"), fixture).expect("write fixture");

    let mut encoder = RpgEncoder::new().expect("encoder");
    let result = encoder.encode(&dir_path).expect("encode");
    (result.graph, dir_path)
}

/// Encode the scenario fixture and return just the graph.
pub fn encoded_scenario_graph() -> RpgGraph {
    encoded_scenario_with_dir().0
}

/// Encode the multi-file fixture (mod_a.rs + mod_b.rs). Returns (graph, repo_dir).
/// mod_a declares `pub mod b;` and calls `b::helper_in_b()`.
pub fn encoded_multi_file_with_dir() -> (RpgGraph, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.keep();

    let mod_a = include_str!("../fixtures/multi_file/mod_a.rs");
    let mod_b = include_str!("../fixtures/multi_file/mod_b.rs");
    std::fs::create_dir_all(dir_path.join("src")).expect("mkdir src");
    // mod_a.rs declares `pub mod b;` so mod_b must be named b.rs.
    std::fs::write(dir_path.join("src/mod_a.rs"), mod_a).expect("write mod_a");
    std::fs::write(dir_path.join("src/b.rs"), mod_b).expect("write b.rs");
    // Also write lib.rs to pull them together.
    std::fs::write(dir_path.join("src/lib.rs"), "pub mod mod_a;\npub mod b;\n")
        .expect("write lib");

    let mut encoder = RpgEncoder::new().expect("encoder");
    let result = encoder.encode(&dir_path).expect("encode");
    (result.graph, dir_path)
}

/// Encode a language fixture from rpg-encoder/tests/fixtures/{lang}/{file}.
/// Generic over the existing single-file pattern: writes to src/{file}, encodes.
pub fn encoded_language_fixture(lang: &str, file: &str) -> (RpgGraph, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.keep();

    let fixture_path = format!("../../../rpg-encoder/tests/fixtures/{lang}/{file}");
    let fixture = include_str_str(&fixture_path);
    std::fs::create_dir_all(dir_path.join("src")).expect("mkdir src");
    std::fs::write(dir_path.join("src").join(file), fixture).expect("write fixture");

    let mut encoder = RpgEncoder::new().expect("encoder");
    let result = encoder.encode(&dir_path).expect("encode");
    (result.graph, dir_path)
}

/// Workaround: include_str! needs a literal path, so we can't pass a variable.
/// This helper dispatches to the right include_str! based on lang+file.
fn include_str_str(path: &str) -> &'static str {
    match path {
        "../../../rpg-encoder/tests/fixtures/python/basic.py" => {
            include_str!("../../../rpg-encoder/tests/fixtures/python/basic.py")
        }
        "../../../rpg-encoder/tests/fixtures/go/basic.go" => {
            include_str!("../../../rpg-encoder/tests/fixtures/go/basic.go")
        }
        "../../../rpg-encoder/tests/fixtures/javascript/basic.js" => {
            include_str!("../../../rpg-encoder/tests/fixtures/javascript/basic.js")
        }
        _ => panic!("no include_str! mapping for {path}"),
    }
}

/// Encode the ffi.rs fixture (has #[no_mangle] exports + extern "C" imports).
pub fn encoded_ffi_graph() -> RpgGraph {
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.keep();

    let fixture = include_str!("../../../rpg-encoder/tests/fixtures/rust/ffi.rs");
    std::fs::create_dir_all(dir_path.join("src")).expect("mkdir src");
    std::fs::write(dir_path.join("src/main.rs"), fixture).expect("write fixture");

    let mut encoder = RpgEncoder::new().expect("encoder");
    let result = encoder.encode(&dir_path).expect("encode");
    result.graph
}

/// Build an RpgService backed by a given graph + repo_dir. The service has
/// a live AppState with snapshot (for reverse_deps / repo_dir) so all tools
/// work including get_source and get_impact.
pub fn service_for_graph(graph: RpgGraph, repo_dir: PathBuf) -> RpgService {
    let mut snapshot = RpgSnapshot::new("test", &repo_dir);
    snapshot.graph = graph;
    snapshot.build_reverse_deps();
    let config = McpConfig {
        workspace: repo_dir.clone(),
        data_dir: repo_dir.join(".rpg"),
        hash_mode: rpg_mcp::state::HashMode::Mtime,
        semantic: false,
    };
    let registry = Arc::new(ParserRegistry::new());
    let state = AppState::new(config, snapshot, registry);
    RpgService::new(Arc::new(state))
}

/// Build an RpgService from the scenario fixture.
pub fn service_for_scenario() -> RpgService {
    let (graph, repo_dir) = encoded_scenario_with_dir();
    service_for_graph(graph, repo_dir)
}

/// Build an RpgService from the basic fixture.
pub fn service_for_basic() -> RpgService {
    let (graph, repo_dir) = encoded_fixture_with_dir();
    service_for_graph(graph, repo_dir)
}

/// Build an RpgService from the ffi fixture.
pub fn service_for_ffi() -> RpgService {
    let graph = encoded_ffi_graph();
    // ffi fixture was written to a temp dir that's now gone; get_source
    // won't work but graph queries will. Use a dummy repo_dir.
    service_for_graph(graph, PathBuf::new())
}

/// Extract the JSON object from a CallToolResult (the tool's return value).
pub fn result_json(result: &rmcp::model::CallToolResult) -> serde_json::Value {
    let text = result
        .content
        .first()
        .and_then(|c| c.raw.as_text().map(|t| t.text.as_str()))
        .unwrap_or("{}");
    serde_json::from_str(text).unwrap_or_else(|_| serde_json::Value::Null)
}

/// Find a node by exact name. Returns the first match's NodeId.
pub fn find_node(graph: &RpgGraph, name: &str) -> NodeId {
    graph
        .nodes()
        .find(|n| n.name == name)
        .map(|n| n.id)
        .unwrap_or_else(|| panic!("node '{name}' not found in fixture"))
}

/// BFS over a specific edge type from `start`. `upstream = true` follows
/// incoming; `false` follows outgoing. Returns all reachable node IDs within
/// `depth` hops. Mirrors what explore_graph/get_callers/get_callees do.
pub fn bfs_edge_type(
    graph: &RpgGraph,
    start: NodeId,
    upstream: bool,
    depth: usize,
    edge_type: EdgeType,
) -> HashSet<NodeId> {
    let mut visited: HashSet<NodeId> = HashSet::new();
    visited.insert(start);
    let mut frontier: Vec<(NodeId, usize)> = vec![(start, 0)];

    while let Some((node_id, d)) = frontier.pop() {
        if d >= depth {
            continue;
        }
        let neighbors: Vec<(NodeId, &rpg_encoder::Edge)> = if upstream {
            graph.edges_to(node_id)
        } else {
            graph.edges_from(node_id)
        };
        for (nbr, edge) in neighbors {
            if edge.edge_type != edge_type {
                continue;
            }
            if visited.insert(nbr) {
                frontier.push((nbr, d + 1));
            }
        }
    }

    visited
}

/// BFS over Calls edges from `start`. `upstream = true` follows incoming
/// (callers); `false` follows outgoing (callees). Returns all reachable node
/// IDs within `depth` hops. Mirrors what get_callers/get_callees do.
pub fn bfs_calls(graph: &RpgGraph, start: NodeId, upstream: bool, depth: usize) -> HashSet<NodeId> {
    bfs_edge_type(graph, start, upstream, depth, EdgeType::Calls)
}

/// Extract the first node id from a search_nodes result. Eliminates the
/// duplicated `search["nodes"][0]["id"].as_u64().expect(...)` pattern across
/// 15+ test sites.
pub fn first_node_id(search: &serde_json::Value) -> u64 {
    search["nodes"][0]["id"]
        .as_u64()
        .expect("search returned no nodes")
}
