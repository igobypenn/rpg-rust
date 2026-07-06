//! Edge-type filter matrix + edge-case tests.
//! Validates that every EdgeType variant is parseable by get_edges/explore_graph,
//! and exercises boundary conditions (empty graph, single node, depth clamping,
//! overflow).

mod common;

use rpg_encoder::{EdgeType, RpgGraph};
use serde_json::json;

fn params(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_value(v).unwrap_or_default()
}

// ============================================================================
// EdgeType filter matrix: every variant is parseable
// ============================================================================

const ALL_EDGE_TYPES: &[(&str, EdgeType)] = &[
    ("contains", EdgeType::Contains),
    ("calls", EdgeType::Calls),
    ("imports", EdgeType::Imports),
    ("references", EdgeType::References),
    ("implements", EdgeType::Implements),
    ("extends", EdgeType::Extends),
    ("depends_on", EdgeType::DependsOn),
    ("defines", EdgeType::Defines),
    ("uses", EdgeType::Uses),
    ("uses_type", EdgeType::UsesType),
    ("ffi_binding", EdgeType::FfiBinding),
    ("implements_feature", EdgeType::ImplementsFeature),
    ("belongs_to_feature", EdgeType::BelongsToFeature),
    ("contains_feature", EdgeType::ContainsFeature),
    ("belongs_to_component", EdgeType::BelongsToComponent),
];

#[tokio::test]
async fn edge_filter_matrix_all_variants_parseable() {
    let service = common::service_for_scenario();

    for (name, _expected_type) in ALL_EDGE_TYPES {
        let result = service
            .get_edges(params(json!({ "edge_type": name, "limit": 5 })))
            .await
            .unwrap_or_else(|_| panic!("get_edges with edge_type={name} should not error"));
        let j = common::result_json(&result);
        // Should return a valid count (0 if no edges of that type exist).
        assert!(
            j["count"].as_u64().is_some(),
            "edge_type={name}: should return a count"
        );
    }
}

#[tokio::test]
async fn edge_filter_contains_returns_only_contains() {
    let service = common::service_for_scenario();
    let result = common::result_json(
        &service
            .get_edges(params(json!({ "edge_type": "contains" })))
            .await
            .expect("ok"),
    );
    let edges = result["edges"].as_array().expect("edges");
    for e in edges {
        assert_eq!(e["type"].as_str(), Some("contains"));
    }
}

#[tokio::test]
async fn edge_filter_aliases_work() {
    let service = common::service_for_scenario();
    // "ffi" and "ffi_binding" should both map to FfiBinding.
    for alias in &["ffi", "ffi_binding"] {
        let _ = service
            .get_edges(params(json!({ "edge_type": alias, "limit": 5 })))
            .await
            .unwrap_or_else(|_| panic!("alias {alias} should not error"));
    }
    // "depends" should map to DependsOn.
    let _ = service
        .get_edges(params(json!({ "edge_type": "depends" })))
        .await
        .expect("'depends' alias should work");
}

// ============================================================================
// Edge cases: empty graph, single node, boundary params
// ============================================================================

#[tokio::test]
async fn empty_graph_search_returns_empty() {
    let service = common::service_for_graph(RpgGraph::new(), std::path::PathBuf::new());
    let result = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "anything" })))
            .await
            .expect("ok"),
    );
    assert_eq!(result["count"].as_u64(), Some(0));
}

#[tokio::test]
async fn empty_graph_summary_has_zero_counts() {
    let service = common::service_for_graph(RpgGraph::new(), std::path::PathBuf::new());
    let result = common::result_json(
        &service
            .get_graph_summary(params(json!({})))
            .await
            .expect("ok"),
    );
    assert_eq!(result["total_nodes"].as_u64(), Some(0));
    assert_eq!(result["total_edges"].as_u64(), Some(0));
}

#[tokio::test]
async fn empty_graph_skeleton_returns_empty_files() {
    let service = common::service_for_graph(RpgGraph::new(), std::path::PathBuf::new());
    let result = common::result_json(
        &service.get_skeleton(params(json!({}))).await.expect("ok"),
    );
    let files = result["files"].as_array().expect("files array");
    assert!(files.is_empty());
}

#[tokio::test]
async fn explore_graph_depth_zero_clamps_to_one() {
    let service = common::service_for_scenario();
    let search = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "lib.rs" })))
            .await
            .expect("ok"),
    );
    let file_id = search["nodes"][0]["id"].as_u64().expect("file id");

    // depth=0 should clamp to 1 — still returns the start node at minimum.
    let result = common::result_json(
        &service
            .explore_graph(params(json!({
                "start_node": file_id,
                "direction": "downstream",
                "depth": 0,
            })))
            .await
            .expect("ok"),
    );
    assert!(
        result["node_count"].as_u64().unwrap_or(0) >= 1,
        "depth=0 clamped to 1 should still return the start node"
    );
}

#[tokio::test]
async fn get_source_context_lines_overflow_no_panic() {
    let service = common::service_for_scenario();
    let search = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "process_payment" })))
            .await
            .expect("ok"),
    );
    let id = search["nodes"][0]["id"].as_u64().expect("id");

    // context_lines=9999 should saturate, not panic.
    let result = service
        .get_source(params(json!({ "node_id": id, "context_lines": 9999 })))
        .await;
    assert!(result.is_ok(), "huge context_lines should not error");
}

#[tokio::test]
async fn get_edges_huge_limit_no_panic() {
    let service = common::service_for_scenario();
    let result = service
        .get_edges(params(json!({ "limit": 18446744073709551615u64 })))
        .await;
    // Should not panic; may return all edges.
    assert!(result.is_ok(), "huge limit should not error");
}

#[tokio::test]
async fn explore_graph_depth_above_max_clamps() {
    let service = common::service_for_scenario();
    let search = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "lib.rs" })))
            .await
            .expect("ok"),
    );
    let file_id = search["nodes"][0]["id"].as_u64().expect("file id");

    // depth=999 should clamp to 10 — depth_reached must not exceed 10.
    let result = common::result_json(
        &service
            .explore_graph(params(json!({
                "start_node": file_id,
                "direction": "downstream",
                "depth": 999,
            })))
            .await
            .expect("ok"),
    );
    let depth_reached = result["depth_reached"].as_u64().unwrap_or(0);
    assert!(
        depth_reached <= 10,
        "depth should clamp to 10, got {depth_reached}"
    );
}

// ============================================================================
// NodeCategory filter matrix
// ============================================================================

const ALL_CATEGORIES: &[&str] = &[
    "repository",
    "directory",
    "file",
    "module",
    "type",
    "function",
    "variable",
    "import",
    "constant",
    "field",
    "parameter",
    "feature",
    "component",
    "functional_centroid",
];

#[tokio::test]
async fn category_filter_matrix_all_parseable() {
    let service = common::service_for_scenario();

    for cat in ALL_CATEGORIES {
        // search_nodes with category filter — should not error for any valid category.
        let result = service
            .search_nodes(params(json!({ "query": ".", "category": cat })))
            .await
            .unwrap_or_else(|_| panic!("category={cat} should not error"));
        let j = common::result_json(&result);
        assert!(
            j["count"].as_u64().is_some(),
            "category={cat}: should return a count"
        );
    }
}

#[tokio::test]
async fn category_filter_function_returns_only_functions() {
    let service = common::service_for_scenario();
    let result = common::result_json(
        &service
            .search_nodes(params(json!({ "query": ".", "category": "function" })))
            .await
            .expect("ok"),
    );
    let nodes = result["nodes"].as_array().expect("nodes");
    for n in nodes {
        assert_eq!(
            n["category"].as_str(),
            Some("function"),
            "all results should be Function category"
        );
    }
}
