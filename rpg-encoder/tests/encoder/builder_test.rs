use rpg_encoder::{GraphBuilder, NodeCategory};
use std::path::Path;

#[test]
fn test_builder_new() {
    let builder = GraphBuilder::new();
    let graph = builder.build();
    assert_eq!(graph.node_count(), 0);
}

#[test]
fn test_builder_with_repo() {
    let builder = GraphBuilder::new().with_repo("my-repo", Path::new("/path/to/repo"));
    let graph = builder.build();

    let repo_nodes: Vec<_> = graph
        .nodes()
        .filter(|n| n.category == NodeCategory::Repository)
        .collect();

    assert_eq!(repo_nodes.len(), 1);
    assert_eq!(repo_nodes[0].name, "my-repo");
}

#[test]
fn test_builder_add_file() {
    let graph = GraphBuilder::new()
        .with_repo("test", Path::new("/test"))
        .add_file(Path::new("src/main.rs"), "rust")
        .build();

    let file_nodes: Vec<_> = graph
        .nodes()
        .filter(|n| n.category == NodeCategory::File)
        .collect();

    assert_eq!(file_nodes.len(), 1);
}

#[test]
fn test_builder_multiple_files() {
    let graph = GraphBuilder::new()
        .with_repo("test", Path::new("/test"))
        .add_file(Path::new("src/main.rs"), "rust")
        .add_file(Path::new("src/lib.rs"), "rust")
        .add_file(Path::new("src/utils.rs"), "rust")
        .build();

    let file_count = graph
        .nodes()
        .filter(|n| n.category == NodeCategory::File)
        .count();

    assert_eq!(file_count, 3);
}

#[test]
fn test_builder_directory_created_implicitly() {
    let graph = GraphBuilder::new()
        .with_repo("test", Path::new("/test"))
        .add_file(Path::new("src/main.rs"), "rust")
        .build();

    let dir_count = graph
        .nodes()
        .filter(|n| n.category == NodeCategory::Directory)
        .count();

    assert_eq!(dir_count, 1);
}

#[test]
fn test_builder_get_file_id() {
    let builder = GraphBuilder::new()
        .with_repo("test", Path::new("/test"))
        .add_file(Path::new("src/main.rs"), "rust");

    let id = builder.get_file_id(Path::new("src/main.rs"));
    assert!(id.is_some());

    let missing = builder.get_file_id(Path::new("src/missing.rs"));
    assert!(missing.is_none());
}

#[test]
fn test_builder_duplicate_file() {
    let graph = GraphBuilder::new()
        .with_repo("test", Path::new("/test"))
        .add_file(Path::new("src/main.rs"), "rust")
        .add_file(Path::new("src/main.rs"), "rust")
        .build();

    let file_count = graph
        .nodes()
        .filter(|n| n.category == NodeCategory::File)
        .count();

    assert_eq!(file_count, 1);
}
