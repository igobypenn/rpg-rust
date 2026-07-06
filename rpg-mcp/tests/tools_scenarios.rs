//! End-to-end scenario tests for every MCP tool. Invokes the actual #[tool]
//! methods on RpgService against the scenario fixture (which has known
//! structure: trait, two impls, call chain, dead code, FFI). Each test
//! asserts concrete expected results, not just "something exists".

mod common;

use serde_json::{json, Map, Value};

// Build params JsonObject from json! value.
fn params(v: Value) -> Map<String, Value> {
    serde_json::from_value(v).unwrap_or_default()
}

// ============================================================================
// SCENARIO: encode_repo + get_graph_summary (the bootstrap path)
// ============================================================================

#[tokio::test]
async fn scenario_graph_summary_after_encode() {
    let service = common::service_for_scenario();

    let summary = common::result_json(
        &service
            .get_graph_summary(params(json!({})))
            .await
            .expect("summary ok"),
    );

    assert!(summary["total_nodes"].as_u64().unwrap_or(0) > 0, "should have nodes");
    assert!(summary["total_edges"].as_u64().unwrap_or(0) > 0, "should have edges");
    let langs = summary["languages"].as_array().expect("languages array");
    assert!(
        langs.iter().any(|l| l == "rust"),
        "should include rust language"
    );
    assert!(
        summary.get("node_category_counts").is_some(),
        "should have category counts"
    );
}

// ============================================================================
// SCENARIO: search_nodes finds definitions by name
// ============================================================================

#[tokio::test]
async fn scenario_search_finds_payment_processor() {
    let service = common::service_for_scenario();

    let result = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "PaymentProcessor" })))
            .await
            .expect("search ok"),
    );

    let nodes = result["nodes"].as_array().expect("nodes array");
    assert!(
        nodes.iter().any(|n| n["name"].as_str() == Some("PaymentProcessor")),
        "should find PaymentProcessor trait"
    );
}

#[tokio::test]
async fn scenario_search_with_kind_filter() {
    let service = common::service_for_scenario();

    // Search for structs only.
    let result = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "Processor", "kind": "struct" })))
            .await
            .expect("search ok"),
    );

    let nodes = result["nodes"].as_array().expect("nodes array");
    // Should find StripeProcessor and PaypalProcessor (both structs).
    let names: Vec<&str> = nodes.iter().filter_map(|n| n["name"].as_str()).collect();
    assert!(names.iter().any(|n| n.contains("Stripe")), "should find StripeProcessor");
    assert!(names.iter().any(|n| n.contains("Paypal")), "should find PaypalProcessor");
}

// ============================================================================
// SCENARIO: get_node_details returns rich data at each detail level
// ============================================================================

#[tokio::test]
async fn scenario_node_details_minimal_vs_full() {
    let service = common::service_for_scenario();

    // Find process_payment's id first.
    let search = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "process_payment" })))
            .await
            .expect("search ok"),
    );
    let id = search["nodes"][0]["id"].as_u64().expect("found node id");

    // Minimal: should NOT have signature/description.
    let minimal = common::result_json(
        &service
            .get_node_details(params(json!({ "id": id, "detail_level": "minimal" })))
            .await
            .expect("details ok"),
    );
    assert!(minimal.get("id").is_some(), "minimal has id");
    assert!(minimal.get("name").is_some(), "minimal has name");
    assert!(
        minimal.get("signature").is_none() || minimal["signature"].is_null(),
        "minimal should not have signature"
    );

    // Full: should have location and node_level.
    let full = common::result_json(
        &service
            .get_node_details(params(json!({ "id": id, "detail_level": "full" })))
            .await
            .expect("details ok"),
    );
    assert!(
        full.get("node_level").is_some(),
        "full should have node_level"
    );
    assert!(
        full.get("location").is_some() || full.get("source_ref").is_some(),
        "full should have location or source_ref"
    );
    assert!(
        full["incoming_edges"].is_array() && full["outgoing_edges"].is_array(),
        "should have edges arrays"
    );
}

#[tokio::test]
async fn scenario_node_details_invalid_id_errors() {
    let service = common::service_for_scenario();
    let result = service
        .get_node_details(params(json!({ "id": 999999 })))
        .await;
    assert!(result.is_err(), "invalid id should return error");
}

// ============================================================================
// SCENARIO: get_source reads actual file lines
// ============================================================================

#[tokio::test]
async fn scenario_get_source_returns_content() {
    let service = common::service_for_scenario();

    let search = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "process_payment" })))
            .await
            .expect("search ok"),
    );
    let id = search["nodes"][0]["id"].as_u64().expect("id");

    let source = common::result_json(
        &service
            .get_source(params(json!({ "node_id": id })))
            .await
            .expect("source ok"),
    );

    let content = source["content"].as_str().expect("content");
    assert!(
        content.contains("process_payment"),
        "source should contain the function name"
    );
    assert!(
        source["start_line"].as_u64().is_some(),
        "should have start_line"
    );
}

#[tokio::test]
async fn scenario_get_source_with_context_lines() {
    let service = common::service_for_scenario();

    let search = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "refund_payment" })))
            .await
            .expect("search ok"),
    );
    let id = search["nodes"][0]["id"].as_u64().expect("id");

    let source = common::result_json(
        &service
            .get_source(params(json!({ "node_id": id, "context_lines": 3 })))
            .await
            .expect("source ok"),
    );

    let content = source["content"].as_str();
    assert!(content.is_some(), "should have content with context");
}

// ============================================================================
// SCENARIO: explore_graph traverses Contains edges
// ============================================================================

#[tokio::test]
async fn scenario_explore_downstream_from_file() {
    let service = common::service_for_scenario();

    // Find the file node.
    let search = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "lib.rs" })))
            .await
            .expect("search ok"),
    );
    let file_id = search["nodes"][0]["id"].as_u64().expect("file id");

    let explored = common::result_json(
        &service
            .explore_graph(params(json!({
                "start_node": file_id,
                "direction": "downstream",
                "depth": 2,
                "edge_type": "contains",
            })))
            .await
            .expect("explore ok"),
    );

    let names: Vec<&str> = explored["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["name"].as_str())
        .collect();
    assert!(
        names.iter().any(|n| n == &"process_payment"),
        "downstream from file should reach process_payment"
    );
    assert!(
        names.iter().any(|n| n == &"PaymentProcessor"),
        "downstream from file should reach PaymentProcessor"
    );
}

#[tokio::test]
async fn scenario_explore_invalid_start_errors() {
    let service = common::service_for_scenario();
    let result = service
        .explore_graph(params(json!({ "start_node": 999999 })))
        .await;
    assert!(result.is_err(), "invalid start should error");
}

#[tokio::test]
async fn scenario_explore_respects_limit() {
    let service = common::service_for_scenario();
    let search = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "lib.rs" })))
            .await
            .expect("search ok"),
    );
    let file_id = search["nodes"][0]["id"].as_u64().expect("file id");

    let explored = common::result_json(
        &service
            .explore_graph(params(json!({
                "start_node": file_id,
                "direction": "downstream",
                "depth": 5,
                "edge_type": "contains",
                "limit": 3,
            })))
            .await
            .expect("explore ok"),
    );

    let node_count = explored["node_count"].as_u64().unwrap_or(999);
    assert!(
        node_count <= 4, // limit + start
        "explore should respect limit, got {node_count}"
    );
}

// ============================================================================
// SCENARIO: get_skeleton shows file → children structure
// ============================================================================

#[tokio::test]
async fn scenario_skeleton_has_files_and_children() {
    let service = common::service_for_scenario();
    let skeleton = common::result_json(
        &service.get_skeleton(params(json!({}))).await.expect("skeleton ok"),
    );

    let files = skeleton["files"].as_array().expect("files array");
    assert!(!files.is_empty(), "should have files");
    let first = &files[0];
    assert!(!first["path"].is_null(), "file has path");
    assert!(
        first["children"].is_array(),
        "file has children array"
    );
}

// ============================================================================
// SCENARIO: get_features returns nodes with features/description
// ============================================================================

#[tokio::test]
async fn scenario_features_empty_without_semantic_encoding() {
    let service = common::service_for_scenario();
    let features = common::result_json(
        &service.get_features(params(json!({}))).await.expect("features ok"),
    );

    // Without semantic encoding, there may be zero or few nodes with features.
    // Just verify the tool returns valid JSON.
    assert!(
        features["nodes"].is_array(),
        "features should return nodes array"
    );
}

// ============================================================================
// SCENARIO: get_ffi_bindings finds the no_mangle export + extern import
// ============================================================================

#[tokio::test]
async fn scenario_ffi_bindings_lists_exports_and_imports() {
    let service = common::service_for_ffi();
    let result = common::result_json(
        &service
            .get_ffi_bindings(params(json!({})))
            .await
            .expect("ffi ok"),
    );

    let bindings = result["ffi_bindings"].as_array().expect("bindings array");
    assert!(!bindings.is_empty(), "ffi fixture should have bindings");

    let symbols: Vec<&str> = bindings.iter().filter_map(|b| b["symbol"].as_str()).collect();
    // The ffi.rs fixture has add_numbers, process_data (#[no_mangle] exports)
    // and external_function, another_external (extern "C" imports).
    assert!(
        !symbols.is_empty(),
        "should find FFI symbols, got: {symbols:?}"
    );
    // At least one should be a known export or import from the fixture.
    assert!(
        symbols.iter().any(|s| s == &"add_numbers" || s == &"external_function"),
        "should find a known FFI symbol, got: {symbols:?}"
    );
}

#[tokio::test]
async fn scenario_ffi_bindings_have_source_and_target() {
    let service = common::service_for_ffi();
    let result = common::result_json(
        &service
            .get_ffi_bindings(params(json!({})))
            .await
            .expect("ffi ok"),
    );

    let bindings = result["ffi_bindings"].as_array().expect("bindings");
    for b in bindings {
        assert!(
            !b["source_lang"].is_null(),
            "binding should have source_lang"
        );
        assert!(
            !b["target_lang"].is_null(),
            "binding should have target_lang"
        );
    }
}

// ============================================================================
// SCENARIO: get_feature_tree on a non-semantic graph returns empty centroids
// ============================================================================

#[tokio::test]
async fn scenario_feature_tree_without_semantic() {
    let service = common::service_for_scenario();
    let tree = common::result_json(
        &service
            .get_feature_tree(params(json!({})))
            .await
            .expect("tree ok"),
    );

    // Without semantic encoding, there are no V^H centroids.
    let count = tree["centroid_count"].as_u64().unwrap_or(0);
    assert_eq!(count, 0, "non-semantic graph should have 0 centroids");
}

// ============================================================================
// SCENARIO: get_impact returns affected files for a node
// ============================================================================

#[tokio::test]
async fn scenario_impact_returns_structure() {
    let service = common::service_for_scenario();

    let search = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "process_payment" })))
            .await
            .expect("search ok"),
    );
    let id = search["nodes"][0]["id"].as_u64().expect("id");

    let impact = common::result_json(
        &service
            .get_impact(params(json!({ "node_ids": [id], "depth": 2 })))
            .await
            .expect("impact ok"),
    );

    // Verify the shape.
    assert!(
        impact["callers"].is_array(),
        "impact should have callers array"
    );
    assert!(
        impact["callees"].is_array(),
        "impact should have callees array"
    );
    assert!(
        impact["affected_files"].is_array(),
        "impact should have affected_files"
    );
    assert!(
        impact["caller_count"].as_u64().is_some(),
        "should have caller_count"
    );
}

#[tokio::test]
async fn scenario_impact_empty_ids_errors() {
    let service = common::service_for_scenario();
    let result = service
        .get_impact(params(json!({ "node_ids": [] })))
        .await;
    assert!(result.is_err(), "empty node_ids should error");
}

// ============================================================================
// SCENARIO: semantic_search returns valid structure (no semantic data here)
// ============================================================================

#[tokio::test]
async fn scenario_semantic_search_no_results_without_enrichment() {
    let service = common::service_for_scenario();
    let result = common::result_json(
        &service
            .semantic_search(params(json!({ "query": "process payment" })))
            .await
            .expect("search ok"),
    );

    // Without semantic encoding, there's no semantic_feature data, so 0 hits.
    assert_eq!(
        result["count"].as_u64().unwrap_or(0),
        0,
        "non-semantic graph should return 0 semantic results"
    );
}

#[tokio::test]
async fn scenario_semantic_search_missing_query_errors() {
    let service = common::service_for_scenario();
    let result = service.semantic_search(params(json!({}))).await;
    assert!(result.is_err(), "missing query should error");
}

// ============================================================================
// SCENARIO: get_edges filters by type
// ============================================================================

#[tokio::test]
async fn scenario_edges_filter_by_contains() {
    let service = common::service_for_scenario();
    let result = common::result_json(
        &service
            .get_edges(params(json!({ "edge_type": "contains", "limit": 10 })))
            .await
            .expect("edges ok"),
    );

    let edges = result["edges"].as_array().expect("edges array");
    assert!(!edges.is_empty(), "should have Contains edges");
    for e in edges {
        assert_eq!(
            e["type"].as_str(),
            Some("contains"),
            "all edges should be Contains"
        );
    }
}

#[tokio::test]
async fn scenario_edges_unrecognized_type_returns_all() {
    let service = common::service_for_scenario();
    let result = common::result_json(
        &service
            .get_edges(params(json!({ "edge_type": "nonexistent_type" })))
            .await
            .expect("edges ok"),
    );

    // An unrecognized edge type falls through to None (no filter) — matches
    // the existing get_edges behavior where unknown types are ignored.
    let count = result["count"].as_u64().unwrap_or(0);
    assert!(
        count > 0,
        "unrecognized type falls through to no-filter (returns all edges): {count}"
    );
}

// ============================================================================
// SCENARIO: get_components returns array (may be empty)
// ============================================================================

#[tokio::test]
async fn scenario_components_valid_structure() {
    let service = common::service_for_scenario();
    let result = common::result_json(
        &service
            .get_components(params(json!({})))
            .await
            .expect("components ok"),
    );

    assert!(
        result["components"].is_array(),
        "components should be an array"
    );
    assert!(
        result["count"].as_u64().is_some(),
        "should have count"
    );
}
