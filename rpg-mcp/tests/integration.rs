use std::path::Path;

use rpg_encoder::{encoder::ValidationReport, RpgEncoder};
use rpg_mcp::state::{compute_dir_hash, HashMode};

fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR should have a parent (workspace root)")
        .to_path_buf()
}

fn encoder_src_dir() -> std::path::PathBuf {
    workspace_root().join("rpg-encoder/src")
}

#[test]
fn test_directory_hash_is_valid_sha256() {
    let dir = encoder_src_dir();
    assert!(dir.exists(), "Test dir should exist: {:?}", dir);

    let hash = compute_dir_hash(&dir, HashMode::Mtime).expect("compute_dir_hash should succeed");
    assert!(!hash.is_empty(), "Hash should not be empty");
    assert_eq!(
        hash.len(),
        64,
        "SHA-256 hex should be 64 chars, got {}",
        hash.len()
    );
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "Hash should be valid hex"
    );

    let content_hash =
        compute_dir_hash(&dir, HashMode::Content).expect("content hash should succeed");
    assert_eq!(content_hash.len(), 64, "Content hash should be 64 chars");
    assert!(
        content_hash.chars().all(|c| c.is_ascii_hexdigit()),
        "Content hash should be valid hex"
    );
}

#[test]
fn test_encoding_produces_graph() {
    let dir = encoder_src_dir();
    assert!(dir.exists(), "Test dir should exist: {:?}", dir);

    let mut encoder = RpgEncoder::new().expect("Failed to create encoder");
    let result = encoder.encode(&dir).expect("Encoding should succeed");

    assert!(result.graph.node_count() > 0, "Graph should have nodes");
    assert!(result.graph.edge_count() > 0, "Graph should have edges");
    assert!(result.files_processed > 0, "Should have processed files");

    println!(
        "Encoded {:?}: {} files, {} nodes, {} edges",
        dir,
        result.files_processed,
        result.graph.node_count(),
        result.graph.edge_count()
    );
}

#[test]
fn test_validation_report_from_graph() {
    let dir = encoder_src_dir();
    assert!(dir.exists(), "Test dir should exist: {:?}", dir);

    let mut encoder = RpgEncoder::new().expect("Failed to create encoder");
    let result = encoder.encode(&dir).expect("Encoding should succeed");

    let report = ValidationReport::from_graph(&result.graph);

    assert!(
        report.total_nodes > 0,
        "ValidationReport total_nodes should be > 0"
    );
    assert!(
        report.total_edges > 0,
        "ValidationReport total_edges should be > 0"
    );
    assert!(
        !report.node_category_counts.is_empty(),
        "Should have node categories"
    );
    assert!(
        !report.edge_type_counts.is_empty(),
        "Should have edge types"
    );

    println!("\n=== Validation Report (rpg-encoder/src) ===");
    println!(
        "Nodes: {}  Edges: {}",
        report.total_nodes, report.total_edges
    );
    println!(
        "Import resolution: {:.1}%",
        report.import_resolution_rate * 100.0
    );
    for (cat, count) in &report.node_category_counts {
        println!("  node {:?}: {}", cat, count);
    }
    for (etype, count) in &report.edge_type_counts {
        println!("  edge {:?}: {}", etype, count);
    }
    for warning in &report.warnings {
        println!("WARNING: {}", warning);
    }
}
