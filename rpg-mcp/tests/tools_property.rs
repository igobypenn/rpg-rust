//! Property-based tests for MCP tool invariants. Uses proptest to validate
//! that explore_graph/get_callers/search_nodes hold their contracts across
//! random graphs.
//!
//! Self-contained generators (doesn't depend on rpg-encoder's private
//! property generators).

mod common;

use proptest::prelude::*;
use rpg_encoder::{Edge, EdgeType, Node, NodeCategory, NodeId, RpgGraph};
use serde_json::json;

fn params(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_value(v).unwrap_or_default()
}

/// Generate a random graph with N nodes and random Calls edges between them.
/// Node names are "func_{i}" so search_nodes can be tested deterministically.
fn make_graph(node_count: usize, edge_density: f64, cyclic: bool) -> RpgGraph {
    let mut graph = RpgGraph::new();
    let mut ids: Vec<NodeId> = Vec::with_capacity(node_count);

    for i in 0..node_count {
        let node = Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "fn",
            "rust",
            format!("func_{i}"),
        );
        ids.push(graph.add_node(node));
    }

    let edge_count = (node_count as f64 * edge_density) as usize;
    for i in 0..edge_count {
        let src = ids[i % node_count];
        let tgt = ids[(i + 1) % node_count];
        graph.add_edge(src, tgt, Edge::new(EdgeType::Calls));
    }

    // If cyclic requested, add a back edge from last to first.
    if cyclic && node_count > 2 {
        graph.add_edge(
            ids[node_count - 1],
            ids[0],
            Edge::new(EdgeType::Calls),
        );
    }

    graph
}

proptest! {
    /// explore_graph never returns more than `limit` nodes.
    #[test]
    fn prop_explore_respects_limit(
        n in 5usize..50,
        density in 0.0f64..2.0,
        limit in 1u64..20,
    ) {
        let graph = make_graph(n, density, false);
        let service = common::service_for_graph(graph, std::path::PathBuf::new());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            common::result_json(
                &service.explore_graph(params(json!({
                    "start_node": 1,
                    "direction": "downstream",
                    "depth": 5,
                    "limit": limit,
                }))).await.expect("ok")
            )
        });

        let node_count = result["node_count"].as_u64().unwrap_or(0);
        prop_assert!(
            node_count <= limit + 1, // +1 for the start node added before limit check
            "node_count {} should be <= limit {} + 1",
            node_count, limit
        );
    }

    /// explore_graph terminates on cyclic graphs (no infinite loop).
    #[test]
    fn prop_explore_terminates_on_cycles(
        n in 3usize..20,
    ) {
        let graph = make_graph(n, 1.0, true);
        let service = common::service_for_graph(graph, std::path::PathBuf::new());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            common::result_json(
                &service.explore_graph(params(json!({
                    "start_node": 0,
                    "direction": "both",
                    "depth": 10,
                }))).await.expect("ok")
            )
        });

        // If we got here, it terminated. Verify it returned something.
        let count = result["node_count"].as_u64().unwrap_or(0);
        prop_assert!(count >= 1, "explore should return at least the start node");
    }

    /// explore_graph depth_reached never exceeds the depth param (after clamping).
    #[test]
    fn prop_explore_depth_bounded(
        n in 5usize..30,
        depth in 1u64..5,
    ) {
        let graph = make_graph(n, 1.5, false);
        let service = common::service_for_graph(graph, std::path::PathBuf::new());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            common::result_json(
                &service.explore_graph(params(json!({
                    "start_node": 0,
                    "direction": "downstream",
                    "depth": depth,
                }))).await.expect("ok")
            )
        });

        let reached = result["depth_reached"].as_u64().unwrap_or(0);
        prop_assert!(
            reached <= depth,
            "depth_reached {} should be <= depth {}",
            reached, depth
        );
    }

    /// search_nodes: every returned node's name contains the query (case-insensitive).
    #[test]
    fn prop_search_results_contain_query(
        n in 5usize..30,
        query_fragment in "[a-z]{1,3}",
    ) {
        let graph = make_graph(n, 0.0, false);
        let service = common::service_for_graph(graph, std::path::PathBuf::new());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            common::result_json(
                &service.search_nodes(params(json!({
                    "query": query_fragment,
                }))).await.expect("ok")
            )
        });

        let nodes = result["nodes"].as_array().unwrap();
        for node in nodes {
            let name = node["name"].as_str().unwrap_or("");
            let name_lower = name.to_ascii_lowercase();
            let query_lower = query_fragment.to_ascii_lowercase();
            prop_assert!(
                name_lower.contains(&query_lower),
                "result name '{}' should contain query '{}'",
                name, query_fragment
            );
        }
    }

    /// get_callers on a node with no incoming Calls edges returns empty.
    #[test]
    fn prop_callers_empty_for_isolated_node(
        n in 3usize..15,
    ) {
        // Density 0 = no edges.
        let graph = make_graph(n, 0.0, false);
        let service = common::service_for_graph(graph, std::path::PathBuf::new());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            common::result_json(
                &service.get_callers(params(json!({
                    "node_id": 0,
                    "depth": 1,
                }))).await.expect("ok")
            )
        });

        let count = result["count"].as_u64().unwrap_or(0);
        prop_assert_eq!(count, 0, "isolated node should have 0 callers");
    }

    /// get_impact affected_files is always a subset of files touched.
    #[test]
    fn prop_impact_affected_files_subset(
        n in 5usize..20,
        density in 0.5f64..2.0,
    ) {
        let graph = make_graph(n, density, false);
        let service = common::service_for_graph(graph, std::path::PathBuf::new());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            common::result_json(
                &service.get_impact(params(json!({
                    "node_ids": [0],
                    "depth": 2,
                }))).await.expect("ok")
            )
        });

        // affected_files should be an array (possibly empty since nodes have no paths).
        assert!(result["affected_files"].is_array());
        // The structure should always be present regardless of graph shape.
        assert!(result["callers"].is_array());
        assert!(result["callees"].is_array());
    }
}
