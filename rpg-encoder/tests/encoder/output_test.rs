use rpg_encoder::{to_json, GraphBuilder, RpgGraph};
use std::path::Path;

fn create_test_graph() -> RpgGraph {
    GraphBuilder::new()
        .with_repo("test-repo", Path::new("/test"))
        .add_file(Path::new("src/main.rs"), "rust")
        .add_file(Path::new("src/lib.rs"), "rust")
        .build()
}

#[test]
fn test_to_json_contains_nodes() {
    let graph = create_test_graph();
    let json = to_json(&graph).unwrap();

    assert!(json.contains("\"nodes\""));
}

#[test]
fn test_to_json_contains_edges() {
    let graph = create_test_graph();
    let json = to_json(&graph).unwrap();

    assert!(json.contains("\"edges\""));
}

#[test]
fn test_to_json_is_valid_json() {
    let graph = create_test_graph();
    let json = to_json(&graph).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_object());
    assert!(parsed["nodes"].is_array());
    assert!(parsed["edges"].is_array());
}

#[test]
fn test_to_json_metadata_included() {
    let graph = create_test_graph();
    let json = to_json(&graph).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["metadata"].is_object());
}

#[test]
fn test_json_has_repo_name() {
    let graph = create_test_graph();
    let json = to_json(&graph).unwrap();

    assert!(json.contains("test-repo"));
}

#[test]
fn test_empty_graph() {
    let graph = RpgGraph::new();
    let json = to_json(&graph).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["nodes"].as_array().unwrap().len(), 0);
    assert_eq!(parsed["edges"].as_array().unwrap().len(), 0);
}
