//! Property tests for graph operations

use super::generators::*;
use proptest::prelude::*;
use rpg_encoder::{Edge, EdgeType, NodeId, RpgGraph};

proptest! {
    #[test]
    fn add_remove_is_idempotent(
        nodes in proptest::collection::vec(any_node(), 1..50)
    ) {
        let mut graph = RpgGraph::new();
        let initial_count = graph.node_count();

        let ids: Vec<_> = nodes.iter().map(|n| graph.add_node(n.clone())).collect();
        prop_assert_eq!(graph.node_count(), initial_count + ids.len());

        for id in &ids {
            graph.remove_node(*id);
        }
        prop_assert_eq!(graph.node_count(), initial_count);
    }

    #[test]
    fn node_count_invariant(
        adds in proptest::collection::vec(any_node(), 0..20),
        remove_indices in proptest::collection::vec(any::<usize>(), 0..10)
    ) {
        let mut graph = RpgGraph::new();
        let mut ids: Vec<_> = Vec::new();

        for node in adds {
            ids.push(graph.add_node(node));
        }

        let after_add = graph.node_count();
        let mut removed = 0;

        for idx in remove_indices {
            if idx < ids.len() {
                if graph.remove_node(ids[idx]).is_some() {
                    removed += 1;
                }
            }
        }

        prop_assert_eq!(graph.node_count(), after_add - removed);
    }

    #[test]
    fn edge_count_never_exceeds_possible(
        nodes in proptest::collection::vec(any_node(), 2..20),
        edges in proptest::collection::vec((any::<usize>(), any::<usize>(), any_edge_type()), 0..30)
    ) {
        let mut graph = RpgGraph::new();
        let ids: Vec<_> = nodes.iter().map(|n| graph.add_node(n.clone())).collect();

        for (src_idx, tgt_idx, edge_type) in edges {
            if src_idx < ids.len() && tgt_idx < ids.len() && src_idx != tgt_idx {
                graph.add_edge(ids[src_idx], ids[tgt_idx], Edge::new(edge_type));
            }
        }

        let max_edges = ids.len() * (ids.len() - 1);
        prop_assert!(graph.edge_count() <= max_edges);
    }

    #[test]
    fn serialization_roundtrip_json(graph in any_graph(20)) {
        let json = rpg_encoder::to_json_compact(&graph).unwrap();
        let deserialized: RpgGraph = serde_json::from_str(&json).unwrap();

        prop_assert_eq!(graph.node_count(), deserialized.node_count());
    }

    #[test]
    fn get_node_returns_added_node(nodes in proptest::collection::vec(any_node(), 1..30)) {
        let mut graph = RpgGraph::new();
        let mut ids_and_nodes: Vec<(NodeId, _)> = Vec::new();

        for node in nodes {
            let id = graph.add_node(node.clone());
            ids_and_nodes.push((id, node));
        }

        for (id, original) in &ids_and_nodes {
            if let Some(retrieved) = graph.get_node(*id) {
                prop_assert_eq!(retrieved.name, original.name);
                prop_assert_eq!(retrieved.category, original.category);
            }
        }
    }

    #[test]
    fn remove_node_removes_connected_edges(
        nodes in proptest::collection::vec(any_node(), 3..15),
        edges in proptest::collection::vec((any::<usize>(), any::<usize>()), 0..20)
    ) {
        let mut graph = RpgGraph::new();
        let ids: Vec<_> = nodes.iter().map(|n| graph.add_node(n.clone())).collect();

        for (src_idx, tgt_idx) in edges {
            if src_idx < ids.len() && tgt_idx < ids.len() && src_idx != tgt_idx {
                graph.add_edge(ids[src_idx], ids[tgt_idx], Edge::new(EdgeType::Calls));
            }
        }

        let edges_before = graph.edge_count();

        if !ids.is_empty() {
            let to_remove = ids[0];
            graph.remove_node(to_remove);

            let node_removed = graph.get_node(to_remove).is_none();
            prop_assert!(node_removed);
        }
    }
}
