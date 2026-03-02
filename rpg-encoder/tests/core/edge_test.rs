use rpg_encoder::{Edge, EdgeType};

#[test]
fn test_edge_types() {
    let types = vec![
        EdgeType::Contains,
        EdgeType::Imports,
        EdgeType::Calls,
        EdgeType::Defines,
        EdgeType::UsesType,
        EdgeType::FfiBinding,
    ];

    assert_eq!(types.len(), 6);
}

#[test]
fn test_edge_creation() {
    let edge = Edge::new(EdgeType::Calls);

    assert_eq!(edge.edge_type, EdgeType::Calls);
}

#[test]
fn test_edge_from_type() {
    let edge: Edge = EdgeType::Imports.into();

    assert_eq!(edge.edge_type, EdgeType::Imports);
}

#[test]
fn test_edge_metadata() {
    let edge =
        Edge::new(EdgeType::Calls).with_metadata("call_count", serde_json::Value::Number(5.into()));

    assert!(edge.metadata.contains_key("call_count"));
}
