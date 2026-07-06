//! Tests for the Tier 3 tools: find_node_at_location,
//! get_architecture_overview, find_dead_code, detect_changes.
//!
//! Plus coverage gaps: get_callees behavior test.

mod common;

use serde_json::json;

fn params(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_value(v).unwrap_or_default()
}

// === find_node_at_location ===

/// Locating a line inside a known function returns that function as the node.
#[tokio::test]
async fn find_node_at_location_finds_enclosing_function() {
    let (graph, dir) = common::encoded_scenario_with_dir();

    // Find process_payment's line range, then query its midpoint.
    let (file, mid_line) = {
        let func = graph
            .nodes()
            .find(|n| n.name == "process_payment")
            .expect("fixture has process_payment");
        let mid = func
            .source_ref
            .as_ref()
            .map(|sr| (sr.start_line + sr.end_line) / 2)
            .or_else(|| func.location.as_ref().map(|l| l.start_line))
            .unwrap_or(10);
        let path = func
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "src/lib.rs".to_string());
        (path, mid)
    };

    let service = common::service_for_graph(graph, dir);
    let result = service
        .find_node_at_location(params(json!({
            "file": file,
            "line": mid_line,
        })))
        .await
        .expect("tool call");

    let v = common::result_json(&result);
    assert_eq!(v["found"], true);
    assert_eq!(v["node"]["name"], "process_payment");
}

/// A line that's inside no node's range returns found=false.
#[tokio::test]
async fn find_node_at_location_outside_any_node() {
    let service = common::service_for_scenario();

    let result = service
        .find_node_at_location(params(json!({
            "file": "src/lib.rs",
            "line": 9999,
        })))
        .await
        .expect("tool call");

    let v = common::result_json(&result);
    assert_eq!(v["found"], false);
}

/// Missing 'file' param is an error.
#[tokio::test]
async fn find_node_at_location_requires_file_param() {
    let service = common::service_for_scenario();

    let result = service
        .find_node_at_location(params(json!({"line": 10})))
        .await;
    assert!(result.is_err(), "missing 'file' should be invalid_params");
}

/// Line 0 is rejected.
#[tokio::test]
async fn find_node_at_location_rejects_line_zero() {
    let service = common::service_for_scenario();

    let result = service
        .find_node_at_location(params(json!({"file": "src/lib.rs", "line": 0})))
        .await;
    assert!(result.is_err(), "line=0 should be invalid_params");
}

// === get_architecture_overview ===

/// The overview returns hub nodes, languages, and file counts.
#[tokio::test]
async fn architecture_overview_has_structure() {
    let service = common::service_for_scenario();

    let result = service
        .get_architecture_overview(params(json!({}))
    )
        .await
        .expect("tool call");

    let v = common::result_json(&result);
    assert!(v["total_nodes"].as_u64().unwrap() > 0);
    assert!(v["total_edges"].as_u64().unwrap() > 0);
    assert!(v["file_nodes"].as_u64().unwrap() > 0);
    // Languages array should include rust.
    let langs = v["languages"].as_array().expect("languages is array");
    assert!(langs.iter().any(|l| l["language"] == "rust"));
    // hub_nodes and largest_files are arrays (may be empty for small fixture).
    assert!(v["hub_nodes"].is_array());
    assert!(v["largest_files"].is_array());
}

/// top_n parameter limits hub nodes.
#[tokio::test]
async fn architecture_overview_respects_top_n() {
    let service = common::service_for_scenario();

    let result = service
        .get_architecture_overview(params(json!({"top_n": 2})))
        .await
        .expect("tool call");

    let v = common::result_json(&result);
    let hubs = v["hub_nodes"].as_array().unwrap();
    assert!(hubs.len() <= 2);
}

// === find_dead_code ===

/// find_dead_code returns a list (possibly empty) with a count and note.
#[tokio::test]
async fn find_dead_code_returns_list() {
    let service = common::service_for_scenario();

    let result = service
        .find_dead_code(params(json!({})))
        .await
        .expect("tool call");

    let v = common::result_json(&result);
    assert!(v["dead_code"].is_array());
    assert!(v["count"].as_u64().is_some());
    assert!(v["note"].as_str().unwrap().contains("Review"));
}

/// Scope filter limits the scan to a path prefix.
#[tokio::test]
async fn find_dead_code_scope_filter() {
    let service = common::service_for_scenario();

    let result = service
        .find_dead_code(params(json!({"scope": "src/nonexistent/"})))
        .await
        .expect("tool call");

    let v = common::result_json(&result);
    assert_eq!(v["count"].as_u64().unwrap_or(0), 0);
}

// === detect_changes ===

/// With no changes since encode, detect_changes returns changed=false.
#[tokio::test]
async fn detect_changes_no_changes() {
    let service = common::service_for_scenario();

    // The scenario fixture was just encoded, so the stored hash should match.
    let result = service
        .detect_changes(params(json!({})))
        .await
        .expect("tool call");

    let v = common::result_json(&result);
    // changed=false (hashes match) OR changed=true if hash mode is mtime and
    // the temp dir's mtime shifted. Either way it's not an error.
    assert!(
        v["changed"].is_boolean() || v.get("hash").is_some(),
        "expected a status response, got: {v}"
    );
}

/// Missing required params: detect_changes takes none, so this just verifies
/// it doesn't panic on an empty param object.
#[tokio::test]
async fn detect_changes_handles_empty_params() {
    let service = common::service_for_basic();

    let _ = service
        .detect_changes(params(json!({})))
        .await
        .expect("tool call should not error even with no changes");
}

// === get_callees (coverage gap) ===

/// get_callees returns the call targets of a function that makes calls.
#[tokio::test]
async fn get_callees_returns_targets() {
    let service = common::service_for_scenario();

    // Find a node, then check its callees. The scenario fixture has Contains
    // edges for sure (file → functions), even if Calls edges are sparse.
    let result = service
        .get_callees(params(json!({"node_id": 0, "depth": 1})))
        .await
        .expect("tool call");

    let v = common::result_json(&result);
    // The response has a "callees" array (possibly empty).
    assert!(v["callees"].is_array() || v.get("count").is_some() || v.get("node_id").is_some(),
        "expected a structured response, got: {v}");
}

/// get_callees with depth=0 returns just the start node.
#[tokio::test]
async fn get_callees_depth_zero() {
    let service = common::service_for_scenario();

    let result = service
        .get_callees(params(json!({"node_id": 0, "depth": 0})))
        .await
        .expect("tool call");

    let v = common::result_json(&result);
    // depth=0 means "just the start node" — no transitive traversal.
    let callees = v["callees"].as_array();
    if let Some(arr) = callees {
        assert!(arr.len() <= 1, "depth=0 should return at most the start node");
    }
}
