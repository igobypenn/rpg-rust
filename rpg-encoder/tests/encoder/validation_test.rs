use rpg_encoder::{RpgEncoder, ValidationReport};
use std::path::Path;

#[test]
fn test_self_encoding_quality() {
    let mut encoder = RpgEncoder::new().expect("Failed to create encoder");
    let result = encoder.encode(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("Encode failed");
    let report = ValidationReport::from_graph(&result.graph);

    println!("\n=== rpg-encoder Validation Report ===");
    println!("Nodes: {}  Edges: {}", report.total_nodes, report.total_edges);
    println!("Import resolution: {:.1}%", report.import_resolution_rate * 100.0);
    println!("Calls: {}  Implements: {}  FFI: {}",
        report.call_edge_count, report.implements_edge_count, report.ffi_edge_count);
    for (etype, count) in &report.edge_type_counts {
        println!("  {}: {}", etype, count);
    }
    for warning in &report.warnings {
        println!("WARNING: {}", warning);
    }

    assert!(result.graph.node_count() > 2000, "Should have 2000+ nodes for self-encoding");
    assert!(result.graph.edge_count() > 3000, "Should have 3000+ edges");
}

#[test]
fn test_self_encoding_baseline() {
    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert!(
        result.parse_errors.len() < result.total_files() / 10,
        "Less than 10% parse error rate, got {} errors from {} files",
        result.parse_errors.len(),
        result.total_files()
    );
}
