use rpg_encoder::{Edge, EdgeType, Node, NodeCategory, NodeId, RpgGraph};
use std::path::PathBuf;

#[test]
fn test_graph_remove_nonexistent_node() {
    let mut graph = RpgGraph::new();
    let result = graph.remove_node(NodeId::new(999));
    assert!(result.is_none());
}

#[test]
fn test_graph_remove_node_twice() {
    let mut graph = RpgGraph::new();
    let id = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "test",
    ));

    let first = graph.remove_node(id);
    assert!(first.is_some());

    let second = graph.remove_node(id);
    assert!(second.is_none(), "Removing twice should return None");
}

#[test]
fn test_graph_remove_node_with_edges() {
    let mut graph = RpgGraph::new();

    let n1 = graph.add_node(
        Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "a")
            .with_path(PathBuf::from("test.rs")),
    );
    let n2 = graph.add_node(
        Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "b")
            .with_path(PathBuf::from("test.rs")),
    );

    graph.add_edge(n1, n2, Edge::new(EdgeType::Calls));
    assert_eq!(graph.edge_count(), 1);

    let removed = graph.remove_file_nodes(&PathBuf::from("test.rs"));
    assert!(!removed.is_empty());
}

#[test]
fn test_graph_find_node_by_path_not_found() {
    let graph = RpgGraph::new();
    let result = graph.find_node_by_path(&PathBuf::from("nonexistent.rs"));
    assert!(result.is_none());
}

#[test]
fn test_graph_find_node_by_name_multiple() {
    let mut graph = RpgGraph::new();

    graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "test",
    ));
    graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Type,
        "struct",
        "rust",
        "test",
    ));

    let found = graph.find_node_by_name("test", None);
    assert!(found.is_some());

    let found_fn = graph.find_node_by_name("test", Some(NodeCategory::Function));
    assert!(found_fn.is_some());
    assert_eq!(found_fn.unwrap().category, NodeCategory::Function);
}

#[test]
fn test_graph_children_of_leaf() {
    let mut graph = RpgGraph::new();

    let parent = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Module,
        "mod",
        "rust",
        "parent",
    ));
    let child = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "child",
    ));

    graph.add_edge(parent, child, Edge::new(EdgeType::Contains));

    let children_of_child = graph.children_of(child);
    assert!(children_of_child.is_empty());
}

#[test]
fn test_graph_children_of_nonexistent() {
    let graph = RpgGraph::new();
    let children = graph.children_of(NodeId::new(999));
    assert!(children.is_empty());
}

#[test]
fn test_graph_update_nonexistent_node() {
    let mut graph = RpgGraph::new();

    let result = graph.update_node_semantics(
        NodeId::new(999),
        vec!["feature".to_string()],
        "description".to_string(),
        "path".to_string(),
    );

    assert!(!result);
}

#[test]
fn test_graph_edges_involving_isolated() {
    let mut graph = RpgGraph::new();

    let isolated = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "isolated",
    ));

    let edges = graph.edges_involving(isolated);
    assert!(edges.is_empty());
}

#[test]
fn test_graph_retain_edges_all() {
    let mut graph = RpgGraph::new();

    let n1 = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "a",
    ));
    let n2 = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "b",
    ));

    graph.add_edge(n1, n2, Edge::new(EdgeType::Calls));

    graph.retain_edges(|_, _, _| true);
    assert_eq!(graph.edge_count(), 1);
}

#[test]
fn test_graph_retain_edges_none() {
    let mut graph = RpgGraph::new();

    let n1 = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "a",
    ));
    let n2 = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "b",
    ));

    graph.add_edge(n1, n2, Edge::new(EdgeType::Calls));

    graph.retain_edges(|_, _, _| false);
    assert_eq!(graph.edge_count(), 0);
}

#[test]
fn test_graph_node_exists_removed() {
    let mut graph = RpgGraph::new();

    let id = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "test",
    ));
    assert!(graph.node_exists(id));

    graph.remove_node(id);
    assert!(!graph.node_exists(id));
}

#[test]
fn test_graph_to_petgraph_empty() {
    let graph = RpgGraph::new();
    let petgraph = graph.to_petgraph();

    assert_eq!(petgraph.node_count(), 0);
    assert_eq!(petgraph.edge_count(), 0);
}

#[test]
fn test_graph_find_node_in_file_not_found() {
    let mut graph = RpgGraph::new();

    graph.add_node(
        Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "test")
            .with_path(PathBuf::from("src/main.rs")),
    );

    let result = graph.find_node_in_file(&PathBuf::from("src/other.rs"), "test");
    assert!(result.is_none());
}

#[test]
fn test_graph_find_node_by_location_no_match() {
    let mut graph = RpgGraph::new();

    graph.add_node(
        Node::new(NodeId::new(0), NodeCategory::Function, "fn", "rust", "test")
            .with_path(PathBuf::from("src/main.rs"))
            .with_location(rpg_encoder::SourceLocation::new(
                PathBuf::from("src/main.rs"),
                10,
                1,
                15,
                2,
            )),
    );

    let result = graph.find_node_by_location(&PathBuf::from("src/main.rs"), 5);
    assert!(result.is_none());
}

#[test]
fn test_graph_large_node_count() {
    let mut graph = RpgGraph::new();

    for i in 0..10000 {
        graph.add_node(Node::new(
            NodeId::new(i),
            NodeCategory::Function,
            "fn",
            "rust",
            format!("fn_{}", i),
        ));
    }

    assert_eq!(graph.node_count(), 10000);
}

#[test]
fn test_graph_large_edge_count() {
    let mut graph = RpgGraph::new();

    let nodes: Vec<_> = (0..1000)
        .map(|i| {
            graph.add_node(Node::new(
                NodeId::new(i),
                NodeCategory::Function,
                "fn",
                "rust",
                format!("fn_{}", i),
            ))
        })
        .collect();

    for i in 0..999 {
        for j in (i + 1)..(i + 10).min(1000) {
            graph.add_edge(nodes[i], nodes[j], Edge::new(EdgeType::Calls));
        }
    }

    assert!(graph.edge_count() > 5000);
}

#[test]
fn test_graph_self_edge() {
    let mut graph = RpgGraph::new();

    let node = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "recursive",
    ));
    graph.add_edge(node, node, Edge::new(EdgeType::Calls));

    assert_eq!(graph.edge_count(), 1);

    let edges = graph.edges_involving(node);
    assert_eq!(edges.len(), 1);
}

#[test]
fn test_graph_duplicate_edges() {
    let mut graph = RpgGraph::new();

    let n1 = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "a",
    ));
    let n2 = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "b",
    ));

    graph.add_edge(n1, n2, Edge::new(EdgeType::Calls));
    graph.add_edge(n1, n2, Edge::new(EdgeType::Calls));

    assert_eq!(graph.edge_count(), 2);
}
