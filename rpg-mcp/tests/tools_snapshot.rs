//! Snapshot tests: locks the JSON output shape of key tools for regression
//! detection. Scrubs volatile NodeId indices before snapshotting.

mod common;

use serde_json::{json, Value};

fn params(v: Value) -> serde_json::Map<String, Value> {
    serde_json::from_value(v).unwrap_or_default()
}

/// Recursively replace volatile values:
/// - "id"/"source"/"target" integers → "[id]"
/// - "path"/"file" absolute strings (starting with /) → "[path]"
///
/// Relative paths (e.g. "src/lib.rs") are NOT scrubbed — they're deterministic
/// and portable across machines, so capturing them in snapshots is correct.
fn scrub_ids(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                let k = key.as_str();
                if (k == "id" || k == "source" || k == "target") && val.is_u64() {
                    *val = Value::String("[id]".to_string());
                } else if k == "path" || k == "file" {
                    if let Value::String(s) = val {
                        // Only scrub absolute paths (migration case). Relative
                        // paths are stable and should be captured as-is.
                        if s.starts_with('/') {
                            *s = "[path]".to_string();
                        }
                    }
                } else {
                    scrub_ids(val);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                scrub_ids(item);
            }
        }
        _ => {}
    }
}

async fn get_process_payment_id(service: &rpg_mcp::service::RpgService) -> u64 {
    let search = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "process_payment" })))
            .await
            .expect("search ok"),
    );
    search["nodes"][0]["id"].as_u64().expect("id")
}

// ============================================================================
// Node detail levels (locks the detail_level ladder)
// ============================================================================

#[tokio::test]
async fn snapshot_node_details_minimal() {
    let service = common::service_for_scenario();
    let id = get_process_payment_id(&service).await;

    let mut j = common::result_json(
        &service
            .get_node_details(params(json!({ "id": id, "detail_level": "minimal" })))
            .await
            .expect("ok"),
    );
    scrub_ids(&mut j);
    insta::assert_json_snapshot!("node_details_minimal", j);
}

#[tokio::test]
async fn snapshot_node_details_summary() {
    let service = common::service_for_scenario();
    let id = get_process_payment_id(&service).await;

    let mut j = common::result_json(
        &service
            .get_node_details(params(json!({ "id": id, "detail_level": "summary" })))
            .await
            .expect("ok"),
    );
    scrub_ids(&mut j);
    insta::assert_json_snapshot!("node_details_summary", j);
}

#[tokio::test]
async fn snapshot_node_details_full() {
    let service = common::service_for_scenario();
    let id = get_process_payment_id(&service).await;

    let mut j = common::result_json(
        &service
            .get_node_details(params(json!({ "id": id, "detail_level": "full" })))
            .await
            .expect("ok"),
    );
    scrub_ids(&mut j);
    insta::assert_json_snapshot!("node_details_full", j);
}

// ============================================================================
// Tool output shapes
// ============================================================================

#[tokio::test]
async fn snapshot_graph_summary_shape() {
    let service = common::service_for_scenario();
    let mut j = common::result_json(
        &service.get_graph_summary(params(json!({}))).await.expect("ok"),
    );
    // Redact volatile counts — lock the shape, not the values.
    j["total_nodes"] = json!("[count]");
    j["total_edges"] = json!("[count]");
    j["edge_type_counts"] = json!("[counts]");
    j["node_category_counts"] = json!("[counts]");
    j["call_edge_count"] = json!("[count]");
    j["implements_edge_count"] = json!("[count]");
    j["ffi_edge_count"] = json!("[count]");
    j["import_resolution_rate"] = json!("[rate]");
    j["warnings"] = json!("[warnings]");
    insta::assert_json_snapshot!("graph_summary_shape", j);
}

#[tokio::test]
async fn snapshot_ffi_bindings_shape() {
    let service = common::service_for_ffi();
    let mut j = common::result_json(
        &service.get_ffi_bindings(params(json!({}))).await.expect("ok"),
    );
    scrub_ids(&mut j);
    j["count"] = json!("[count]");
    insta::assert_json_snapshot!("ffi_bindings_shape", j);
}

#[tokio::test]
async fn snapshot_skeleton_shape() {
    let service = common::service_for_scenario();
    let mut j = common::result_json(
        &service.get_skeleton(params(json!({}))).await.expect("ok"),
    );
    scrub_ids(&mut j);
    insta::assert_json_snapshot!("skeleton_shape", j);
}
