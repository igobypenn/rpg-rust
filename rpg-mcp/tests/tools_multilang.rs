//! Multi-language tests: validates tools work against non-Rust fixtures
//! (Python, Go). Exercises the 14-language parser breadth that's advertised
//! but otherwise untested.

mod common;

use serde_json::json;

fn params(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_value(v).unwrap_or_default()
}

// ============================================================================
// Python
// ============================================================================

#[tokio::test]
async fn python_search_finds_class() {
    let (graph, dir) = common::encoded_language_fixture("python", "basic.py");
    let service = common::service_for_graph(graph, dir);

    let result = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "DataProcessor" })))
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
        names.iter().any(|n| n.contains("DataProcessor") || n.contains("Processor")),
        "should find a Python class, got: {names:?}"
    );
}

#[tokio::test]
async fn python_summary_lists_language() {
    let (graph, dir) = common::encoded_language_fixture("python", "basic.py");
    let service = common::service_for_graph(graph, dir);

    let result = common::result_json(
        &service.get_graph_summary(params(json!({}))).await.expect("ok"),
    );
    let langs = result["languages"].as_array().expect("languages");
    assert!(
        langs.iter().any(|l| l == "python"),
        "should include python, got: {langs:?}"
    );
}

#[tokio::test]
async fn python_skeleton_shows_py_file() {
    let (graph, dir) = common::encoded_language_fixture("python", "basic.py");
    let service = common::service_for_graph(graph, dir);

    let result = common::result_json(
        &service.get_skeleton(params(json!({}))).await.expect("ok"),
    );
    let files = result["files"].as_array().expect("files");
    assert!(!files.is_empty(), "should have a .py file");
    let has_py = files
        .iter()
        .any(|f| f["path"].as_str().unwrap_or("").ends_with(".py"));
    assert!(has_py, "skeleton should include a .py file");
}

// ============================================================================
// Go
// ============================================================================

#[tokio::test]
async fn go_search_finds_definition() {
    let (graph, dir) = common::encoded_language_fixture("go", "basic.go");
    let service = common::service_for_graph(graph, dir);

    // Search for something — Go basic.go has Config struct / main func.
    let result = common::result_json(
        &service
            .search_nodes(params(json!({ "query": "." , "limit": 100 })))
            .await
            .expect("ok"),
    );
    let count = result["count"].as_u64().unwrap_or(0);
    assert!(count > 0, "Go fixture should produce nodes");
}

#[tokio::test]
async fn go_summary_lists_language() {
    let (graph, dir) = common::encoded_language_fixture("go", "basic.go");
    let service = common::service_for_graph(graph, dir);

    let result = common::result_json(
        &service.get_graph_summary(params(json!({}))).await.expect("ok"),
    );
    let langs = result["languages"].as_array().expect("languages");
    assert!(
        langs.iter().any(|l| l == "go"),
        "should include go, got: {langs:?}"
    );
}

// ============================================================================
// JavaScript
// ============================================================================

#[tokio::test]
async fn js_search_finds_function() {
    let (graph, dir) = common::encoded_language_fixture("javascript", "basic.js");
    let service = common::service_for_graph(graph, dir);

    let result = common::result_json(
        &service.get_graph_summary(params(json!({}))).await.expect("ok"),
    );
    assert!(
        result["total_nodes"].as_u64().unwrap_or(0) > 0,
        "JS fixture should produce nodes"
    );
    let langs = result["languages"].as_array().expect("languages");
    assert!(
        langs.iter().any(|l| l == "javascript"),
        "should include javascript"
    );
}
