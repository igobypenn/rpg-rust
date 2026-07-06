//! Regression tests for bugs found in the second deep-dive analysis.

mod common;

use serde_json::json;

fn params(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_value(v).unwrap_or_default()
}

// ===========================================================================
// Diff parser: "\ No newline at end of file" must not corrupt line counts
// ===========================================================================

#[test]
fn diff_parser_handles_no_newline_marker() {
    use rpg_mcp::tools::diff::ParsedDiff;

    // A diff with the "\ No newline at end of file" marker.
    let diff = "\
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,4 @@
 fn main() {
+    new_line();
 }
\\ No newline at end of file
";

    let parsed = ParsedDiff::parse(diff);
    assert_eq!(parsed.files.len(), 1, "must parse the file");
    assert_eq!(parsed.files[0].0, "src/lib.rs");

    // The added line should be at line 2 (in the new file). The "\ No newline"
    // marker must NOT be counted as a context line — if it were, line numbers
    // would shift by 1.
    assert_eq!(parsed.files[0].1.len(), 1, "must find 1 changed range");
    assert_eq!(
        parsed.files[0].1[0].start, 2,
        "added line must be at line 2, not shifted by the no-newline marker"
    );
}

// ===========================================================================
// Export: DOT must escape newlines in node names
// ===========================================================================

#[tokio::test]
async fn export_dot_escapes_newlines() {
    let service = common::service_for_scenario();

    let v = common::result_json(
        &service
            .export_graph(params(json!({"format": "dot"})))
            .await
            .unwrap(),
    );
    let output = v["output"].as_str().unwrap();

    // No raw newlines inside quoted labels (they should be \\n).
    // A raw newline inside a label would break Graphviz parsing.
    // We check that no line in the output contains a quoted string split
    // across lines (which would indicate an unescaped newline).
    for line in output.lines() {
        if line.contains("label=\"") {
            // Each label= line should be a single line (no embedded newline).
            assert!(
                line.matches('"').count() >= 2,
                "label line should have matching quotes: {line}"
            );
        }
    }
}

// ===========================================================================
// Export: Cypher must use sequential MATCH (not Cartesian)
// ===========================================================================

#[tokio::test]
async fn export_cypher_uses_sequential_match() {
    let service = common::service_for_scenario();

    let v = common::result_json(
        &service
            .export_graph(params(json!({"format": "cypher"})))
            .await
            .unwrap(),
    );
    let output = v["output"].as_str().unwrap();

    // The old Cartesian form "MATCH (a), (b) WHERE" is O(N²) on Neo4j.
    // The fixed form uses "MATCH (a) WHERE ... MATCH (b) WHERE".
    assert!(
        !output.contains("MATCH (a), (b)"),
        "Cypher must not use Cartesian MATCH — found 'MATCH (a), (b)'"
    );
    assert!(
        output.contains("MATCH (a) WHERE") && output.contains("MATCH (b) WHERE"),
        "Cypher must use sequential MATCH (a) WHERE ... MATCH (b) WHERE"
    );
}

// ===========================================================================
// Memory: next_id includes PID for cross-process uniqueness
// ===========================================================================

#[tokio::test]
async fn memory_ids_include_pid() {
    let service = common::service_for_basic();

    let v = common::result_json(
        &service
            .write_memory(params(json!({"content": "test"})))
            .await
            .unwrap(),
    );
    let id = v["id"].as_str().unwrap();

    // Format: mem_<timestamp>_<pid>_<counter>
    let parts: Vec<&str> = id.split('_').collect();
    assert!(parts.len() >= 4, "id should have at least 4 parts: {id}");
    let pid: u32 = parts[2].parse().expect("pid component should be numeric");
    assert_eq!(pid, std::process::id(), "pid in id should match process id");
}

// ===========================================================================
// Storage: embeddings sidecar lives under data_dir (consolidated layout)
// ===========================================================================

#[tokio::test]
async fn vector_search_sidecar_path_uses_data_dir() {
    let (graph, dir) = common::encoded_scenario_with_dir();
    let sidecar = dir.join(".rpg").join("embeddings.bin");
    std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();

    // Plant a sidecar with one vector.
    use rpg_encoder::{EmbeddingIndex, FlatIndex};
    let mut idx = FlatIndex::new(4);
    let node = graph
        .nodes()
        .find(|n| n.category == rpg_encoder::NodeCategory::Function)
        .unwrap();
    idx.insert(node.id, vec![1.0, 0.0, 0.0, 0.0]);
    idx.save(&sidecar).unwrap();

    // Use env var to point embedding endpoint at a dead port so it doesn't
    // actually try to embed the query (we only test that the sidecar is found).
    std::env::set_var("RPGEN_EMBEDDING_ENDPOINT", "http://127.0.0.1:9/v1");

    let service = common::service_for_graph(graph, dir);
    let v = common::result_json(
        &service
            .vector_search(params(json!({"query": "test"})))
            .await
            .unwrap(),
    );

    // Must NOT be "unavailable" — the sidecar was found at data_dir/embeddings.bin.
    assert_ne!(v["status"], "unavailable", "sidecar should be found at data_dir/embeddings.bin");

    std::env::remove_var("RPGEN_EMBEDDING_ENDPOINT");
}
