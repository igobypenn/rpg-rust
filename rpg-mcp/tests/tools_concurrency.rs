//! Concurrency + state management tests. Validates that concurrent tool
//! calls don't deadlock, and that state updates (encode_repo, update) work.

mod common;

use serde_json::json;

fn params(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_value(v).unwrap_or_default()
}

// ============================================================================
// Concurrent reads (no deadlock)
// ============================================================================

#[tokio::test]
async fn concurrent_reads_no_deadlock() {
    let service = std::sync::Arc::new(common::service_for_scenario());
    let s1 = service.clone();
    let s2 = service.clone();
    let s3 = service.clone();
    let s4 = service.clone();

    // Four simultaneous reads — RwLock should allow all without deadlock.
    let (r1, r2, r3, _r4) = tokio::join!(
        async { s1.search_nodes(params(json!({ "query": "PaymentProcessor" }))).await },
        async { s2.get_graph_summary(params(json!({}))).await },
        async { s3.get_skeleton(params(json!({}))).await },
        async {
            s4.explore_graph(params(json!({
                "start_node": 0u64,
                "direction": "downstream",
                "depth": 2,
            })))
            .await
        },
    );

    // All should succeed (no deadlock, no panic).
    assert!(r1.is_ok(), "concurrent search should succeed");
    assert!(r2.is_ok(), "concurrent summary should succeed");
    assert!(r3.is_ok(), "concurrent skeleton should succeed");
    // r4 may error if node 0 is invalid, but shouldn't deadlock.
    // (Node 0 is the Repository root — explore may return empty or error.)
}

// ============================================================================
// State update
// ============================================================================

#[tokio::test]
async fn state_update_swaps_graph() {
    use rpg_encoder::{Node, NodeCategory, NodeId, RpgGraph, RpgSnapshot};
    use std::sync::Arc;

    // Start with an empty graph.
    let dir = tempfile::tempdir().expect("tempdir");
    let dir_path = dir.keep();
    let config = rpg_mcp::state::McpConfig {
        workspace: dir_path.clone(),
        data_dir: PathBuf::new(),
        hash_mode: rpg_mcp::state::HashMode::Mtime,
        semantic: false,
    };
    let snapshot = RpgSnapshot::new("test", &dir_path);
    let registry = Arc::new(rpg_encoder::ParserRegistry::new());
    let state = Arc::new(rpg_mcp::state::AppState::new(config, snapshot, registry));

    // Verify empty.
    {
        let graph = state.graph.read();
        assert_eq!(graph.node_count(), 0);
    }

    // Build a new graph with one node and update state.
    let mut new_graph = RpgGraph::new();
    new_graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "updated_fn",
    ));
    let mut new_snapshot = RpgSnapshot::new("test", &dir_path);
    new_snapshot.graph = new_graph;

    state.update(new_snapshot);

    // Verify the swap is visible.
    {
        let graph = state.graph.read();
        assert_eq!(graph.node_count(), 1, "graph should have 1 node after update");
    }
}

#[tokio::test]
async fn encode_repo_updates_state() {
    use rpg_encoder::RpgSnapshot;
    use std::sync::Arc;

    // Build a service with an empty graph pointing at the scenario fixture dir.
    let (_graph, dir) = common::encoded_scenario_with_dir();
    let data_dir = dir.join(".rpg-data");

    let config = rpg_mcp::state::McpConfig {
        workspace: dir.clone(),
        data_dir: data_dir.clone(),
        hash_mode: rpg_mcp::state::HashMode::Mtime,
        semantic: false,
    };
    // Start with empty snapshot.
    let snapshot = RpgSnapshot::new("test", &dir);
    let registry = Arc::new(rpg_encoder::ParserRegistry::new());
    let state = Arc::new(rpg_mcp::state::AppState::new(config, snapshot, registry));
    let service = rpg_mcp::service::RpgService::new(state);

    // Verify empty before via a tool.
    let before = common::result_json(
        &service
            .get_graph_summary(params(json!({})))
            .await
            .expect("summary ok"),
    );
    assert_eq!(
        before["total_nodes"].as_u64(),
        Some(0),
        "should start empty"
    );

    // Call encode_repo — this re-encodes and updates state.
    let result = service.encode_repo(params(json!({}))).await;
    assert!(result.is_ok(), "encode_repo should succeed");

    // Verify state now has nodes.
    let summary = common::result_json(
        &service
            .get_graph_summary(params(json!({})))
            .await
            .expect("summary ok"),
    );
    let new_count = summary["total_nodes"].as_u64().unwrap_or(0);
    assert!(
        new_count > 0,
        "state should have nodes after encode_repo"
    );
}

#[tokio::test]
async fn encode_repo_persists_store() {
    use rpg_encoder::RpgSnapshot;
    use std::sync::Arc;

    let (_graph, dir) = common::encoded_scenario_with_dir();
    let data_dir = dir.join(".rpg-data");

    let config = rpg_mcp::state::McpConfig {
        workspace: dir.clone(),
        data_dir: data_dir.clone(),
        hash_mode: rpg_mcp::state::HashMode::Mtime,
        semantic: false,
    };
    let snapshot = RpgSnapshot::new("test", &dir);
    let registry = Arc::new(rpg_encoder::ParserRegistry::new());
    let state = Arc::new(rpg_mcp::state::AppState::new(config, snapshot, registry));
    let service = rpg_mcp::service::RpgService::new(state);

    service
        .encode_repo(params(json!({})))
        .await
        .expect("encode ok");

    // Verify the store persisted a base.json. encode_repo opens/initializes
    // the store at workspace/.rpg/ (not data_dir).
    let base_json = dir.join(".rpg").join("base.json");
    assert!(
        base_json.exists(),
        "encode_repo should persist base.json at {base_json:?}"
    );
}

use std::path::PathBuf;
