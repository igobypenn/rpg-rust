//! Snapshot tests for graph serialization
//!
//! These tests capture graph structure in JSON format for regression testing.

use insta::{assert_json_snapshot, assert_snapshot};
use rpg_encoder::{to_json_compact, Edge, EdgeType, Node, NodeCategory, NodeId, RpgGraph};
use std::path::PathBuf;

fn create_sample_graph() -> RpgGraph {
    let mut graph = RpgGraph::new();

    let n1 = graph.add_node(
        Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "main")
            .with_path(PathBuf::from("src/main.rs")),
    );
    let n2 = graph.add_node(
        Node::new(
            NodeId::new(1),
            NodeCategory::Function,
            "fn",
            "rust",
            "helper",
        )
        .with_path(PathBuf::from("src/lib.rs")),
    );
    let n3 = graph.add_node(
        Node::new(
            NodeId::new(2),
            NodeCategory::Type,
            "struct",
            "rust",
            "Config",
        )
        .with_path(PathBuf::from("src/config.rs")),
    );

    graph.add_edge(n1, n2, Edge::new(EdgeType::Calls));
    graph.add_edge(n1, n3, Edge::new(EdgeType::UsesType));

    graph
}

fn create_complex_graph() -> RpgGraph {
    let mut graph = RpgGraph::new();

    let repo = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Repository,
        "repository",
        "rust",
        "my-project",
    ));

    let src_dir = graph.add_node(
        Node::new(
            NodeId::new(1),
            NodeCategory::Directory,
            "directory",
            "rust",
            "src",
        )
        .with_path(PathBuf::from("src")),
    );

    let main_file = graph.add_node(
        Node::new(
            NodeId::new(2),
            NodeCategory::File,
            "file",
            "rust",
            "main.rs",
        )
        .with_path(PathBuf::from("src/main.rs")),
    );

    let lib_file = graph.add_node(
        Node::new(NodeId::new(3), NodeCategory::File, "file", "rust", "lib.rs")
            .with_path(PathBuf::from("src/lib.rs")),
    );

    let main_fn = graph.add_node(
        Node::new(NodeId::new(4), NodeCategory::Function, "fn", "rust", "main")
            .with_path(PathBuf::from("src/main.rs"))
            .with_signature("fn main() -> ()")
            .with_documentation("Entry point"),
    );

    let helper_fn = graph.add_node(
        Node::new(
            NodeId::new(5),
            NodeCategory::Function,
            "fn",
            "rust",
            "helper",
        )
        .with_path(PathBuf::from("src/lib.rs"))
        .with_signature("fn helper(x: i32) -> i32"),
    );

    let config_struct = graph.add_node(
        Node::new(
            NodeId::new(6),
            NodeCategory::Type,
            "struct",
            "rust",
            "Config",
        )
        .with_path(PathBuf::from("src/lib.rs"))
        .with_documentation("Application configuration"),
    );

    graph.add_edge(repo, src_dir, Edge::new(EdgeType::Contains));
    graph.add_edge(src_dir, main_file, Edge::new(EdgeType::Contains));
    graph.add_edge(src_dir, lib_file, Edge::new(EdgeType::Contains));
    graph.add_edge(main_file, main_fn, Edge::new(EdgeType::Contains));
    graph.add_edge(lib_file, helper_fn, Edge::new(EdgeType::Contains));
    graph.add_edge(lib_file, config_struct, Edge::new(EdgeType::Contains));
    graph.add_edge(main_fn, helper_fn, Edge::new(EdgeType::Calls));
    graph.add_edge(main_fn, config_struct, Edge::new(EdgeType::UsesType));

    graph
}

#[test]
fn snapshot_simple_graph_json() {
    let graph = create_sample_graph();
    assert_json_snapshot!(graph);
}

#[test]
fn snapshot_simple_graph_compact() {
    let graph = create_sample_graph();
    let json = to_json_compact(&graph).unwrap();
    assert_snapshot!(json);
}

#[test]
fn snapshot_complex_graph_json() {
    let graph = create_complex_graph();
    assert_json_snapshot!(graph);
}

#[test]
fn snapshot_complex_graph_compact() {
    let graph = create_complex_graph();
    let json = to_json_compact(&graph).unwrap();
    assert_snapshot!(json);
}
