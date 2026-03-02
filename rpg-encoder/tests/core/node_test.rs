use rpg_encoder::{Node, NodeCategory, NodeId};

#[test]
fn test_node_new() {
    let id = NodeId::new(42);
    let node = Node::new(id, NodeCategory::Function, "function", "rust", "test_fn");

    assert_eq!(node.id, id);
    assert_eq!(node.category, NodeCategory::Function);
    assert_eq!(node.kind, "function");
    assert_eq!(node.language, "rust");
    assert_eq!(node.name, "test_fn");
}

#[test]
fn test_node_categories() {
    assert_ne!(NodeCategory::Function, NodeCategory::Type);
    assert_ne!(NodeCategory::Import, NodeCategory::File);
    assert_ne!(NodeCategory::Module, NodeCategory::Repository);
}

#[test]
fn test_node_with_description() {
    let id = NodeId::new(0);
    let node = Node::new(id, NodeCategory::Function, "function", "rust", "my_fn")
        .with_description("A test function");

    assert_eq!(node.description, Some("A test function".to_string()));
}

#[test]
fn test_node_with_features() {
    let id = NodeId::new(0);
    let node = Node::new(id, NodeCategory::Feature, "feature", "rust", "auth")
        .with_features(vec!["login".to_string(), "logout".to_string()]);

    assert_eq!(node.features.len(), 2);
    assert!(node.features.contains(&"login".to_string()));
}

#[test]
fn test_node_with_metadata() {
    let id = NodeId::new(0);
    let node = Node::new(id, NodeCategory::Type, "struct", "rust", "Config").with_metadata(
        "visibility",
        serde_json::Value::String("public".to_string()),
    );

    assert_eq!(
        node.metadata.get("visibility"),
        Some(&serde_json::Value::String("public".to_string()))
    );
}

#[test]
fn test_node_with_path() {
    let id = NodeId::new(0);
    let node =
        Node::new(id, NodeCategory::File, "file", "rust", "main.rs").with_path("/src/main.rs");

    assert_eq!(node.path, Some(std::path::PathBuf::from("/src/main.rs")));
}
