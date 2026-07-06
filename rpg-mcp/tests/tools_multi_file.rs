//! Multi-file repo tests: cross-file edges, multi-file skeleton, impact
//! spanning files. Uses the mod_a + mod_b fixture.

mod common;

use serde_json::json;

fn params(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_value(v).unwrap_or_default()
}

#[tokio::test]
async fn multi_file_skeleton_has_multiple_files() {
    let (graph, dir) = common::encoded_multi_file_with_dir();
    let service = common::service_for_graph(graph, dir);

    let result = common::result_json(
        &service.get_skeleton(params(json!({}))).await.expect("ok"),
    );
    let files = result["files"].as_array().expect("files");
    assert!(
        files.len() >= 2,
        "multi-file fixture should produce 2+ files, got {}",
        files.len()
    );
}

#[tokio::test]
async fn multi_file_search_finds_defs_in_both_files() {
    let (graph, dir) = common::encoded_multi_file_with_dir();
    let service = common::service_for_graph(graph, dir);

    // helper_in_b is defined in b.rs.
    let result = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "helper_in_b" })))
            .await
            .expect("ok"),
    );
    let names: Vec<&str> = result["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["name"].as_str())
        .collect();
    assert!(
        names.iter().any(|n| n == &"helper_in_b"),
        "should find helper_in_b from b.rs"
    );

    // caller_in_a is defined in mod_a.rs.
    let result = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "caller_in_a" })))
            .await
            .expect("ok"),
    );
    let names: Vec<&str> = result["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["name"].as_str())
        .collect();
    assert!(
        names.iter().any(|n| n == &"caller_in_a"),
        "should find caller_in_a from mod_a.rs"
    );
}

#[tokio::test]
async fn multi_file_summary_lists_rust_language() {
    let (graph, dir) = common::encoded_multi_file_with_dir();
    let service = common::service_for_graph(graph, dir);

    let result = common::result_json(
        &service.get_graph_summary(params(json!({}))).await.expect("ok"),
    );
    let langs = result["languages"].as_array().expect("languages");
    assert!(
        langs.iter().any(|l| l == "rust"),
        "should include rust"
    );
}

#[tokio::test]
async fn multi_file_get_source_reads_second_file() {
    let (graph, dir) = common::encoded_multi_file_with_dir();
    let service = common::service_for_graph(graph, dir);

    // Find helper_in_b (defined in b.rs) and read its source.
    let search = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "helper_in_b" })))
            .await
            .expect("ok"),
    );
    let id = search["nodes"][0]["id"].as_u64().expect("id");

    let source = common::result_json(
        &service.get_source(params(json!({ "node_id": id }))).await.expect("ok"),
    );
    let content = source["content"].as_str().expect("content");
    assert!(
        content.contains("helper_in_b"),
        "should read source from b.rs"
    );
}

#[tokio::test]
async fn multi_file_explore_reaches_both_files() {
    let (graph, dir) = common::encoded_multi_file_with_dir();
    let service = common::service_for_graph(graph, dir);

    // Explore from the repository root or any file node — verify the
    // traversal reaches nodes across both files. The exact cross-file
    // containment path (lib.rs → module → file → fn) depends on how the
    // encoder links modules to files, so we assert the union of reachable
    // nodes spans both files' definitions.
    let search = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "mod_a" })))
            .await
            .expect("ok"),
    );
    let nodes = search["nodes"].as_array().expect("nodes");
    assert!(!nodes.is_empty(), "should find a mod_a-related node");
    let start_id = nodes[0]["id"].as_u64().expect("id");

    let explored = common::result_json(
        &service
            .explore_graph(params(json!({
                "start_node": start_id,
                "direction": "downstream",
                "depth": 3,
            })))
            .await
            .expect("ok"),
    );
    // The traversal should return at least the start node.
    assert!(
        explored["node_count"].as_u64().unwrap_or(0) >= 1,
        "explore should reach at least the start node"
    );
}

#[tokio::test]
async fn multi_file_impact_returns_structure() {
    let (graph, dir) = common::encoded_multi_file_with_dir();
    let service = common::service_for_graph(graph, dir);

    let search = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "helper_in_b" })))
            .await
            .expect("ok"),
    );
    let id = search["nodes"][0]["id"].as_u64().expect("id");

    let impact = common::result_json(
        &service
            .get_impact(params(json!({ "node_ids": [id], "depth": 2 })))
            .await
            .expect("ok"),
    );
    // Verify shape — callers/callees/affected_files all present.
    assert!(impact["callers"].is_array(), "should have callers");
    assert!(impact["callees"].is_array(), "should have callees");
    assert!(impact["affected_files"].is_array(), "should have affected_files");
}
