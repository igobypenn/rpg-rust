use rpg_encoder::{EdgeType, Node, NodeCategory, NodeId, RpgGraph};

#[test]
fn test_graph_new() {
    let graph = RpgGraph::new();
    assert_eq!(graph.node_count(), 0);
    assert_eq!(graph.edge_count(), 0);
}

#[test]
fn test_graph_add_node() {
    let mut graph = RpgGraph::new();
    let _id = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "function",
        "rust",
        "main",
    ));

    assert_eq!(graph.node_count(), 1);
}

#[test]
fn test_graph_add_edge() {
    let mut graph = RpgGraph::new();

    let n1 = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "function",
        "rust",
        "caller",
    ));
    let n2 = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "function",
        "rust",
        "callee",
    ));

    graph.add_edge(n1, n2, EdgeType::Calls.into());

    assert_eq!(graph.edge_count(), 1);
}

#[test]
fn test_graph_node_count() {
    let mut graph = RpgGraph::new();
    assert_eq!(graph.node_count(), 0);

    for i in 0..10 {
        graph.add_node(Node::new(
            NodeId::new(i),
            NodeCategory::Function,
            "function",
            "rust",
            format!("fn_{}", i),
        ));
    }

    assert_eq!(graph.node_count(), 10);
}

#[test]
fn test_graph_edge_count() {
    let mut graph = RpgGraph::new();

    let n1 = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "function",
        "rust",
        "a",
    ));
    let n2 = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "function",
        "rust",
        "b",
    ));
    let n3 = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "function",
        "rust",
        "c",
    ));

    graph.add_edge(n1, n2, EdgeType::Calls.into());
    graph.add_edge(n2, n3, EdgeType::Calls.into());
    graph.add_edge(n1, n3, EdgeType::Calls.into());

    assert_eq!(graph.edge_count(), 3);
}

#[test]
fn test_graph_nodes_iter() {
    let mut graph = RpgGraph::new();

    graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "function",
        "rust",
        "a",
    ));
    graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Type,
        "struct",
        "rust",
        "B",
    ));

    let func_count = graph
        .nodes()
        .filter(|n| n.category == NodeCategory::Function)
        .count();
    let type_count = graph
        .nodes()
        .filter(|n| n.category == NodeCategory::Type)
        .count();

    assert_eq!(func_count, 1);
    assert_eq!(type_count, 1);
}

#[test]
fn test_graph_edges_iter() {
    let mut graph = RpgGraph::new();

    let n1 = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "function",
        "rust",
        "a",
    ));
    let n2 = graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "function",
        "rust",
        "b",
    ));

    graph.add_edge(n1, n2, EdgeType::Calls.into());
    graph.add_edge(n1, n2, EdgeType::Contains.into());

    let calls_count = graph
        .edges()
        .filter(|(_, _, e)| e.edge_type == EdgeType::Calls)
        .count();

    assert_eq!(calls_count, 1);
}
