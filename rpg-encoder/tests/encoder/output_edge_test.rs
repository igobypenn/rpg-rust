use rpg_encoder::{to_json, to_json_compact, Edge, EdgeType, Node, NodeCategory, NodeId, RpgGraph};
use std::path::PathBuf;

#[test]
fn test_to_json_empty_graph() {
    let graph = RpgGraph::new();
    let json = to_json(&graph).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["nodes"].as_array().unwrap().len(), 0);
    assert_eq!(parsed["edges"].as_array().unwrap().len(), 0);
}

#[test]
fn test_to_json_all_node_fields() {
    let mut graph = RpgGraph::new();

    graph.add_node(
        Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "fn",
            "rust",
            "test_fn",
        )
        .with_path(PathBuf::from("src/main.rs"))
        .with_location(rpg_encoder::SourceLocation::new(
            PathBuf::from("src/main.rs"),
            10,
            1,
            20,
            2,
        ))
        .with_metadata("doc".to_string(), serde_json::json!("A function")),
    );

    let json = to_json(&graph).unwrap();

    assert!(json.contains("test_fn"));
    assert!(json.contains("src/main.rs"));
    assert!(json.contains("start_line"));
    assert!(json.contains("doc"));
}

#[test]
fn test_to_json_unicode_content() {
    let mut graph = RpgGraph::new();

    graph.add_node(
        Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "関数")
            .with_path(PathBuf::from("src/файл.rs"))
            .with_metadata("emoji".to_string(), serde_json::json!("🎉")),
    );

    let json = to_json(&graph).unwrap();

    assert!(json.contains("関数"));
    assert!(json.contains("файл"));
    assert!(json.contains("🎉"));
}

#[test]
fn test_to_json_special_characters() {
    let mut graph = RpgGraph::new();

    graph.add_node(
        Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "fn",
            "rust",
            "test\"with'quotes",
        )
        .with_metadata(
            "special".to_string(),
            serde_json::json!("line1\nline2\ttab"),
        ),
    );

    let json = to_json(&graph).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_object());
}

#[test]
fn test_to_json_deep_metadata() {
    let mut graph = RpgGraph::new();

    let mut nested = std::collections::HashMap::new();
    nested.insert("level2".to_string(), serde_json::json!({"level3": "value"}));

    graph.add_node(
        Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "fn",
            "rust",
            "nested_test",
        )
        .with_metadata("nested".to_string(), serde_json::json!(nested)),
    );

    let json = to_json(&graph).unwrap();

    assert!(json.contains("level2"));
    assert!(json.contains("level3"));
}

#[test]
fn test_to_json_roundtrip() {
    let mut graph = RpgGraph::new();

    let n1 = graph.add_node(
        Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "fn",
            "rust",
            "caller",
        )
        .with_path(PathBuf::from("src/caller.rs")),
    );
    let n2 = graph.add_node(
        Node::new(
            NodeId::new(1),
            NodeCategory::Function,
            "fn",
            "rust",
            "callee",
        )
        .with_path(PathBuf::from("src/callee.rs")),
    );

    graph.add_edge(n1, n2, Edge::new(EdgeType::Calls));

    let json = to_json(&graph).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(parsed["edges"].as_array().unwrap().len(), 1);
}

#[test]
fn test_to_json_compact_no_pretty_print() {
    let mut graph = RpgGraph::new();
    graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "test",
    ));

    let compact = to_json_compact(&graph).unwrap();
    let pretty = to_json(&graph).unwrap();

    assert!(compact.len() < pretty.len());
    assert!(!compact.contains('\n') || compact.matches('\n').count() < 2);
}

#[test]
fn test_to_json_all_edge_types() {
    let mut graph = RpgGraph::new();

    let n1 = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "a",
    ));
    let n2 = graph.add_node(Node::new(
        NodeId::new(1),
        NodeCategory::Function,
        "fn",
        "rust",
        "b",
    ));

    graph.add_edge(n1, n2, Edge::new(EdgeType::Calls));
    graph.add_edge(n2, n1, Edge::new(EdgeType::References));
    graph.add_edge(n1, n2, Edge::new(EdgeType::Contains));
    graph.add_edge(n2, n1, Edge::new(EdgeType::DependsOn));

    let json = to_json(&graph).unwrap();

    assert!(json.contains("calls"));
    assert!(json.contains("references"));
    assert!(json.contains("contains"));
    assert!(json.contains("dependson"));
}
