//! Regression tests for critical bugs found in the deep analysis.
//!
//! Each test is named after the bug it prevents. These guard against the most
//! damaging regressions: serialization data loss, graph corruption, and
//! resolution correctness.

use std::path::{Path, PathBuf};

use rpg_encoder::{
    Edge, EdgeType, Node, NodeCategory, NodeId, NodeLevel, RpgGraph, RpgSnapshot,
    SourceLocation,
};
use rpg_encoder::core::SourceRef;

// ===========================================================================
// BUG 1: into_snapshot must preserve ALL Node fields
// ===========================================================================

/// Build a node with every field populated, save → load, assert nothing is lost.
#[test]
fn serialization_preserves_all_node_fields() {
    let mut graph = RpgGraph::new();
    let node = Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "do_thing")
        .with_path(PathBuf::from("src/lib.rs"))
        .with_signature("fn do_thing(x: u32) -> bool")
        .with_description("Does the thing")
        .with_features(vec!["feature_a".to_string(), "feature_b".to_string()])
        .with_feature_path("src/lib.rs::do_thing")
        .with_documentation("/// Does the thing\n/// # Arguments")
        .with_semantic_feature("validates input; returns bool");
    let mut node = node;
    node.location = Some(SourceLocation {
        file: PathBuf::from("src/lib.rs"),
        start_line: 10,
        start_column: 0,
        end_line: 20,
        end_column: 1,
    });
    node.source_ref = Some(SourceRef { start_line: 10, end_line: 20 });
    node.metadata.insert("custom".to_string(), serde_json::json!(42));
    node.node_level = NodeLevel::High; // CRITICAL: must survive round-trip
    graph.add_node(node);

    // Also add an edge with metadata to verify edge metadata survives.
    graph.add_node(Node::new(NodeId::new(1), NodeCategory::Type, "struct", "rust", "Ctx"));
    graph.add_edge(
        NodeId::new(0),
        NodeId::new(1),
        Edge::new(EdgeType::UsesType),
    );

    let mut snapshot = RpgSnapshot::new("test", Path::new("/repo"));
    snapshot.graph = graph;

    // Serialize → deserialize via BaseSnapshot (the .rpg/base.json path).
    let base = rpg_encoder::BaseSnapshot::from_snapshot(&snapshot);
    let json = serde_json::to_string(&base).unwrap();
    let loaded: rpg_encoder::BaseSnapshot = serde_json::from_str(&json).unwrap();
    let restored = loaded.into_snapshot(Path::new("/repo"), "test");

    let n = restored.graph.get_node(NodeId::new(0)).expect("node exists");
    assert_eq!(n.name, "do_thing");
    assert_eq!(n.kind, "fn");
    assert_eq!(n.language, "rust");
    assert_eq!(n.category, NodeCategory::Function);
    assert_eq!(n.path.as_deref(), Some(std::path::Path::new("src/lib.rs")));
    assert_eq!(n.signature.as_deref(), Some("fn do_thing(x: u32) -> bool"));
    assert_eq!(n.description.as_deref(), Some("Does the thing"));
    assert_eq!(n.features, vec!["feature_a".to_string(), "feature_b".to_string()]);
    assert_eq!(n.feature_path.as_deref(), Some("src/lib.rs::do_thing"));
    assert!(n.documentation.as_ref().unwrap().contains("Does the thing"));
    assert_eq!(n.semantic_feature.as_deref(), Some("validates input; returns bool"));
    assert_eq!(n.node_level, NodeLevel::High, "node_level MUST survive round-trip");
    assert_eq!(
        n.location.as_ref().unwrap().start_line, 10,
        "location MUST survive round-trip"
    );
    assert_eq!(
        n.source_ref.as_ref().unwrap().end_line, 20,
        "source_ref MUST survive round-trip"
    );
    assert_eq!(
        n.metadata.get("custom"),
        Some(&serde_json::json!(42)),
        "metadata MUST survive round-trip"
    );

    // Edge must survive too.
    assert_eq!(restored.graph.edge_count(), 1, "edge must survive round-trip");
}

/// Specifically verify V^H centroids survive save/load (node_level=High).
#[test]
fn serialization_preserves_centroid_node_level() {
    let mut graph = RpgGraph::new();
    let mut centroid = Node::new(
        NodeId::new(0),
        NodeCategory::FunctionalCentroid,
        "centroid",
        "rust",
        "AuthModule",
    )
    .with_semantic_feature("Handles authentication");
    centroid.node_level = NodeLevel::High;
    graph.add_node(centroid);

    let mut snapshot = RpgSnapshot::new("test", Path::new("/repo"));
    snapshot.graph = graph;

    let base = rpg_encoder::BaseSnapshot::from_snapshot(&snapshot);
    let loaded: rpg_encoder::BaseSnapshot = serde_json::from_str(&serde_json::to_string(&base).unwrap()).unwrap();
    let restored = loaded.into_snapshot(Path::new("/repo"), "test");

    // The centroid must still be findable via functional_centroids().
    let centroids: Vec<_> = restored.graph.functional_centroids().collect();
    assert_eq!(centroids.len(), 1, "centroid must survive with node_level=High");
    assert_eq!(centroids[0].name, "AuthModule");
    assert_eq!(centroids[0].node_level, NodeLevel::High);
}

// ===========================================================================
// BUG: remove_node swap-remove remap must preserve surviving node access
// ===========================================================================

/// Add A, B, C, D; remove B (middle); assert D (which was swapped) is still
/// reachable by its original NodeId.
#[test]
fn remove_node_surviving_last_node_still_accessible() {
    let mut graph = RpgGraph::new();
    let id_a = graph.add_node(Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "a"));
    let id_b = graph.add_node(Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "b"));
    let id_c = graph.add_node(Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "c"));
    let id_d = graph.add_node(Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "d"));

    // Add edges so the graph isn't trivial.
    graph.add_edge(id_a, id_b, Edge::new(EdgeType::Calls));
    graph.add_edge(id_b, id_c, Edge::new(EdgeType::Calls));
    graph.add_edge(id_c, id_d, Edge::new(EdgeType::Calls));

    // Remove B (a middle node). petgraph swap-removes, so D (the last node)
    // gets moved into B's slot internally.
    graph.remove_node(id_b);

    assert_eq!(graph.node_count(), 3);
    // B is gone.
    assert!(graph.get_node(id_b).is_none());
    // D must still be accessible by its original NodeId.
    let d = graph.get_node(id_d).expect("D must be reachable after B removed");
    assert_eq!(d.name, "d");
    // A and C must also be accessible.
    assert!(graph.get_node(id_a).is_some());
    assert!(graph.get_node(id_c).is_some());
    // The edge A→B should be gone (B was removed).
    let a_edges: Vec<_> = graph.edges_from(id_a);
    assert!(a_edges.iter().all(|(tgt, _)| *tgt != id_b));
}

// ===========================================================================
// PERF: in_degree and has_incoming_of_types are correct
// ===========================================================================

#[test]
fn in_degree_matches_edges_to_len() {
    let mut graph = RpgGraph::new();
    let a = graph.add_node(Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "a"));
    let b = graph.add_node(Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "b"));
    let c = graph.add_node(Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "c"));

    graph.add_edge(a, c, Edge::new(EdgeType::Calls));
    graph.add_edge(b, c, Edge::new(EdgeType::References));

    // c has 2 incoming edges.
    assert_eq!(graph.in_degree(c), 2);
    assert_eq!(graph.in_degree(c), graph.edges_to(c).len());

    // a has 0 incoming.
    assert_eq!(graph.in_degree(a), 0);
}

#[test]
fn has_incoming_of_types_short_circuits_correctly() {
    let mut graph = RpgGraph::new();
    let a = graph.add_node(Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "a"));
    let b = graph.add_node(Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "b"));

    graph.add_edge(a, b, Edge::new(EdgeType::Contains));

    // b has an incoming Contains edge, but NOT a Calls edge.
    assert!(graph.has_incoming_of_types(b, &[EdgeType::Contains]));
    assert!(!graph.has_incoming_of_types(b, &[EdgeType::Calls]));
    assert!(graph.has_incoming_of_types(b, &[EdgeType::Calls, EdgeType::Contains])); // either
    assert!(!graph.has_incoming_of_types(b, &[EdgeType::Calls, EdgeType::References]));
}
