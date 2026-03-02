//! Custom proptest generators for rpg-encoder types

use proptest::prelude::*;
use rpg_encoder::{Edge, EdgeType, Node, NodeCategory, NodeId};
use std::path::PathBuf;

pub fn any_node_id() -> impl Strategy<Value = NodeId> {
    (0..10000u64).prop_map(NodeId::new)
}

pub fn any_node_category() -> impl Strategy<Value = NodeCategory> {
    prop_oneof![
        Just(NodeCategory::Repository),
        Just(NodeCategory::Directory),
        Just(NodeCategory::File),
        Just(NodeCategory::Module),
        Just(NodeCategory::Type),
        Just(NodeCategory::Function),
        Just(NodeCategory::Variable),
        Just(NodeCategory::Import),
        Just(NodeCategory::Constant),
        Just(NodeCategory::Field),
        Just(NodeCategory::Parameter),
        Just(NodeCategory::Feature),
        Just(NodeCategory::Component),
    ]
}

pub fn any_node_kind() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("fn".to_string()),
        Just("struct".to_string()),
        Just("enum".to_string()),
        Just("trait".to_string()),
        Just("impl".to_string()),
        Just("mod".to_string()),
        Just("const".to_string()),
        Just("type".to_string()),
        Just("use".to_string()),
    ]
}

pub fn any_language() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("rust".to_string()),
        Just("python".to_string()),
        Just("go".to_string()),
        Just("javascript".to_string()),
        Just("typescript".to_string()),
        Just("java".to_string()),
        Just("c".to_string()),
        Just("cpp".to_string()),
    ]
}

pub fn any_identifier() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,30}"
}

pub fn any_node() -> impl Strategy<Value = Node> {
    (
        any_node_id(),
        any_node_category(),
        any_node_kind(),
        any_language(),
        any_identifier(),
    )
        .prop_map(|(id, category, kind, language, name)| {
            Node::new(id, category, kind, language, name)
        })
}

pub fn any_node_with_path() -> impl Strategy<Value = Node> {
    (any_node(), any_path()).prop_map(|(mut node, path)| {
        node.path = Some(path);
        node
    })
}

pub fn any_edge_type() -> impl Strategy<Value = EdgeType> {
    prop_oneof![
        Just(EdgeType::Calls),
        Just(EdgeType::Contains),
        Just(EdgeType::Imports),
        Just(EdgeType::References),
        Just(EdgeType::UsesType),
        Just(EdgeType::DependsOn),
        Just(EdgeType::Extends),
        Just(EdgeType::Implements),
        Just(EdgeType::Defines),
        Just(EdgeType::Exports),
    ]
}

pub fn any_edge() -> impl Strategy<Value = Edge> {
    any_edge_type().prop_map(Edge::new)
}

pub fn any_path() -> impl Strategy<Value = PathBuf> {
    ("src/[a-z]{2,8}/[a-z]{2,12}\\.(rs|py|go|js|ts)").prop_map(|s| PathBuf::from(s))
}

pub fn any_graph(nodes: usize) -> impl Strategy<Value = rpg_encoder::RpgGraph> {
    proptest::collection::vec(any_node_with_path(), 0..nodes).prop_map(|nodes| {
        let mut graph = rpg_encoder::RpgGraph::new();
        for node in nodes {
            graph.add_node(node);
        }
        graph
    })
}

pub fn any_graph_with_edges(
    nodes: usize,
    edge_density: f32,
) -> impl Strategy<Value = rpg_encoder::RpgGraph> {
    (
        proptest::collection::vec(any_node_with_path(), 2..nodes),
        proptest::collection::vec(
            (any::<usize>(), any::<usize>(), any_edge_type()),
            0..((nodes as f32 * edge_density) as usize),
        ),
    )
        .prop_map(move |(nodes, edges)| {
            let mut graph = rpg_encoder::RpgGraph::new();
            let ids: Vec<_> = nodes.into_iter().map(|n| graph.add_node(n)).collect();

            for (src_idx, tgt_idx, edge_type) in edges {
                if src_idx < ids.len() && tgt_idx < ids.len() && src_idx != tgt_idx {
                    graph.add_edge(ids[src_idx], ids[tgt_idx], Edge::new(edge_type));
                }
            }

            graph
        })
}

pub fn valid_rust_identifier() -> impl Strategy<Value = String> {
    "[a-z_][a-z0-9_]{0,30}"
}

pub fn rust_function_code() -> impl Strategy<Value = String> {
    (valid_rust_identifier(), valid_rust_identifier()).prop_map(|(name, param)| {
        format!(
            r#"pub fn {}({}: i32) -> i32 {{
    {} + 1
}}"#,
            name, param, param
        )
    })
}

pub fn rust_struct_code() -> impl Strategy<Value = String> {
    (
        valid_rust_identifier(),
        proptest::collection::vec(valid_rust_identifier(), 1..5),
    )
        .prop_map(|(name, fields)| {
            let fields_str: Vec<String> =
                fields.iter().map(|f| format!("    {}: i32,", f)).collect();
            format!(
                r#"pub struct {} {{
{}
}}"#,
                name,
                fields_str.join("\n")
            )
        })
}

pub fn arbitrary_source_code() -> impl Strategy<Value = String> {
    ".*"
}
