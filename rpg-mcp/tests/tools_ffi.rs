//! Tests for the FFI bindings tool logic. Validates against the ffi.rs
//! fixture which has known #[no_mangle] exports and extern "C" imports.

mod common;

use rpg_encoder::{EdgeType, NodeCategory};

#[test]
fn test_ffi_fixture_has_ffi_binding_nodes() {
    let graph = common::encoded_ffi_graph();

    // The ffi fixture should produce FFI binding nodes (kind "ffi_binding").
    let ffi_nodes: Vec<_> = graph
        .nodes()
        .filter(|n| n.kind == "ffi_binding")
        .collect();

    assert!(
        !ffi_nodes.is_empty(),
        "ffi fixture should produce FFI binding nodes"
    );
    println!("FFI binding nodes: {:?}", ffi_nodes.iter().map(|n| &n.name).collect::<Vec<_>>());

    // Should include the extern-imported functions.
    let names: Vec<&str> = ffi_nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(
        names.iter().any(|n| n.contains("external_function") || *n == "add_numbers"),
        "FFI nodes should include known bindings, got: {names:?}"
    );
}

#[test]
fn test_ffi_nodes_are_feature_category() {
    let graph = common::encoded_ffi_graph();
    let ffi_nodes: Vec<_> = graph
        .nodes()
        .filter(|n| n.kind == "ffi_binding")
        .collect();

    for n in &ffi_nodes {
        assert_eq!(
            n.category,
            NodeCategory::Feature,
            "FFI binding '{}' should be Feature category",
            n.name
        );
    }
}

#[test]
fn test_ffi_edges_carry_metadata() {
    let graph = common::encoded_ffi_graph();

    // Find FfiBinding edges and verify they carry ffi_source/ffi_target.
    let ffi_edges: Vec<_> = graph
        .edges()
        .filter(|(_, _, e)| e.edge_type == EdgeType::FfiBinding)
        .collect();

    assert!(
        !ffi_edges.is_empty(),
        "ffi fixture should have FfiBinding edges"
    );

    for (_, _, edge) in &ffi_edges {
        assert!(
            edge.metadata.contains_key("ffi_source"),
            "FfiBinding edge should carry ffi_source metadata"
        );
        assert!(
            edge.metadata.contains_key("ffi_target"),
            "FfiBinding edge should carry ffi_target metadata"
        );
    }
}
