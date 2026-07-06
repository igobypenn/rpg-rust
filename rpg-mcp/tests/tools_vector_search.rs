//! Tests for the vector_search MCP tool.
//!
//! The HTTP embedding endpoint is not exercised here (that's an integration
//! test). We cover:
//! 1. No embedding index + no sidecar → graceful "unavailable" response.
//! 2. Sidecar file exists → the tool proceeds past the unavailable check.
//!
//! The actual embedding/search math is covered by rpg-encoder's
//! embeddings_property and embeddings_persist tests.

mod common;

use serde_json::json;
use rpg_encoder::{EmbeddingIndex, FlatIndex};

fn params(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_value(v).unwrap_or_default()
}

/// When no embedding index exists and no sidecar file is present,
/// vector_search returns a graceful "unavailable" status with a fallback hint.
#[tokio::test]
async fn vector_search_unavailable_without_index() {
    let service = common::service_for_scenario();

    let result = service
        .vector_search(params(json!({ "query": "authentication", "top_k": 5 })))
        .await
        .expect("tool call");

    let v = common::result_json(&result);
    assert_eq!(v["status"], "unavailable");
    assert!(v["error"].as_str().unwrap().contains("no embedding index"));
    assert_eq!(
        v["fallback"],
        "use the semantic_search tool for keyword-based search"
    );
}

/// With a sidecar file present, vector_search proceeds past the unavailable
/// check (it will attempt to hit the embedding endpoint, which doesn't exist
/// in tests — so we only assert the status is no longer "unavailable").
#[tokio::test]
async fn vector_search_proceeds_when_sidecar_exists() {
    // Use a unique dummy port to avoid races with other env-mutating tests.
    std::env::set_var("RPGEN_EMBEDDING_ENDPOINT", "http://127.0.0.1:9/v1");

    let (graph, dir) = common::encoded_scenario_with_dir();

    // Plant a sidecar at <workspace>/.rpg/embeddings.bin with one vector.
    let sidecar = dir.join(".rpg").join("embeddings.bin");
    std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
    let mut idx = FlatIndex::new(4);
    let first_fn = graph
        .nodes()
        .find(|n| n.category == rpg_encoder::NodeCategory::Function)
        .expect("fixture has a function node");
    idx.insert(first_fn.id, vec![1.0, 0.0, 0.0, 0.0]);
    assert_eq!(idx.len(), 1);
    idx.save(&sidecar).unwrap();

    let service = common::service_for_graph(graph, dir);

    let result = service
        .vector_search(params(json!({ "query": "x" })))
        .await
        .expect("tool call");

    let v = common::result_json(&result);
    // No longer "unavailable" — the sidecar was found. The tool will report an
    // embedding error (no endpoint), which is the expected graceful path.
    assert_ne!(v["status"], "unavailable");

    std::env::remove_var("RPGEN_EMBEDDING_ENDPOINT");
}

/// The encode_embeddings tool surfaces a clear error when the embedding
/// endpoint is unreachable (no Qwen3 server in tests).
#[tokio::test]
async fn encode_embeddings_fails_gracefully_without_endpoint() {
    // Use a unique dummy port to avoid races with other env-mutating tests.
    std::env::set_var("RPGEN_EMBEDDING_ENDPOINT", "http://127.0.0.1:9/v1");

    let service = common::service_for_scenario();

    let result = service
        .encode_embeddings(params(json!({})))
        .await
        .expect("tool call");

    let v = common::result_json(&result);
    assert_eq!(v["status"], "error");
    assert!(v["error"].as_str().unwrap().contains("embedding request failed"));

    std::env::remove_var("RPGEN_EMBEDDING_ENDPOINT");
}
