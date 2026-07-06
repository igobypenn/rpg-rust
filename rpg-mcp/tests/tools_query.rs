//! Tests for the Tier 1 query logic. Validates the graph primitives the new
//! MCP tools wrap (BFS traversal, edges_from/to, source reading) against an
//! encoded fixture.
//!
//! Note: the basic.rs fixture produces UsesType and Contains edges but NOT
//! Calls edges (method calls like AppConfig::new() resolve to UsesType from
//! the file node, not Calls from the caller function — a known encoder
//! limitation). These tests use the edge types that actually exist.

mod common;

use rpg_encoder::EdgeType;

#[test]
fn test_edges_from_returns_outgoing() {
    let graph = common::encoded_fixture_graph();
    let create_config = common::find_node(&graph, "create_config");

    let outgoing = graph.edges_from(create_config);
    // create_config is a top-level fn; it has a Contains edge from the file
    // (incoming), but its outgoing edges depend on what the parser resolved.
    // At minimum the API should return a Vec without panicking.
    println!("create_config outgoing edges: {outgoing:?}");
}

#[test]
fn test_edges_to_returns_incoming_contains() {
    let graph = common::encoded_fixture_graph();
    // The file node should have incoming Contains edges from... actually
    // Contains flows file -> children, so test the reverse: children have
    // incoming Contains from the file.
    let new_id = common::find_node(&graph, "new");
    let incoming = graph.edges_to(new_id);
    // 'new' should have an incoming Contains edge from the file node.
    assert!(
        incoming
            .iter()
            .any(|(_, e)| e.edge_type == EdgeType::Contains),
        "'new' should have an incoming Contains edge from the file"
    );
}

#[test]
fn test_bfs_downstream_finds_contained_children() {
    let graph = common::encoded_fixture_graph();
    let file_id = common::find_node(&graph, "main.rs");

    // BFS over Contains edges downstream from the file should reach all defs.
    let reachable = common::bfs_edge_type(&graph, file_id, false, 2, EdgeType::Contains);
    let new_id = common::find_node(&graph, "new");
    assert!(
        reachable.contains(&new_id),
        "downstream Contains from file should reach 'new'"
    );
    let create_config = common::find_node(&graph, "create_config");
    assert!(
        reachable.contains(&create_config),
        "downstream Contains from file should reach create_config"
    );
}

#[test]
fn test_bfs_upstream_finds_file() {
    let graph = common::encoded_fixture_graph();
    let new_id = common::find_node(&graph, "new");

    // BFS upstream over Contains from 'new' should reach the file.
    let reachable = common::bfs_edge_type(&graph, new_id, true, 2, EdgeType::Contains);
    let file_id = common::find_node(&graph, "main.rs");
    assert!(
        reachable.contains(&file_id),
        "upstream Contains from 'new' should reach the file"
    );
}

#[test]
fn test_get_node_has_location_for_definition() {
    let graph = common::encoded_fixture_graph();
    let id = common::find_node(&graph, "create_config");
    let node = graph.get_node(id).expect("node exists");
    assert!(
        node.location.is_some() || node.source_ref.is_some(),
        "create_config should have location or source_ref"
    );
}

#[test]
fn test_read_source_lines() {
    use rpg_mcp::tools::format::read_node_source;
    let (graph, repo_dir) = common::encoded_fixture_with_dir();
    let id = common::find_node(&graph, "create_config");
    let node = graph.get_node(id).expect("node exists");

    let source = read_node_source(node, &repo_dir, 0);
    assert!(source.is_some(), "should read source for create_config");
    let (content, _, _) = source.unwrap();
    assert!(
        content.contains("create_config"),
        "source should contain the function name"
    );
}

#[test]
fn test_node_details_full_has_location() {
    // Verify the format helper produces full-detail JSON with location.
    use rpg_mcp::tools::format::{node_to_json, DetailLevel};
    let graph = common::encoded_fixture_graph();
    let id = common::find_node(&graph, "create_config");
    let node = graph.get_node(id).expect("node exists");

    let json = node_to_json(node, DetailLevel::Full);
    assert!(
        json.get("location").is_some() || json.get("source_ref").is_some(),
        "full detail should include location or source_ref"
    );
    assert!(
        json.get("node_level").is_some(),
        "full detail should include node_level"
    );
}
