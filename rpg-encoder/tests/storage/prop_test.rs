use proptest::prelude::*;
use rpg_encoder::encoder::SerializedNode;
use rpg_encoder::storage::{FilePatch, Patch};
use rpg_encoder::*;
use std::path::PathBuf;
use tempfile::TempDir;

proptest! {
    #[test]
    fn save_load_roundtrip_preserves_graph(
        node_count in 0usize..50,
        edge_count in 0usize..100,
    ) {
        let dir = TempDir::new().unwrap();
        let repo_path = dir.path();

        let mut graph = RpgGraph::new();
        let mut node_ids = Vec::new();

        for i in 0..node_count {
            let node = Node::new(
                NodeId::new(0),
                NodeCategory::Function,
                "function",
                "rust",
                format!("fn_{}", i),
            ).with_path(PathBuf::from(format!("src/mod_{}.rs", i % 5)));
            node_ids.push(graph.add_node(node));
        }

        let actual_edges = edge_count.min(node_ids.len().saturating_sub(1));
        for i in 0..actual_edges {
            graph.add_typed_edge(node_ids[i], node_ids[i + 1], EdgeType::Calls);
        }

        let mut snapshot = RpgSnapshot::new("prop-test", repo_path);
        snapshot.graph = graph;

        let mut store = RpgStore::init(repo_path).unwrap();
        store.save_base(&snapshot).unwrap();

        let loaded = store.load().unwrap();

        prop_assert_eq!(loaded.graph.node_count(), node_count);
        prop_assert_eq!(loaded.graph.edge_count(), actual_edges);
    }

    #[test]
    fn patch_apply_preserves_node_count(
        nodes_before in 1usize..10,
        nodes_after in 1usize..10,
    ) {
        let dir = TempDir::new().unwrap();
        let repo_path = dir.path();

        let mut graph = RpgGraph::new();
        for i in 0..nodes_before {
            graph.add_node(
                Node::new(NodeId::new(0), NodeCategory::Function, "function", "rust", format!("old_{}", i)),
            );
        }

        let mut snapshot = RpgSnapshot::new("test", repo_path);
        snapshot.graph = graph;

        let mut store = RpgStore::init(repo_path).unwrap();
        store.save_base(&snapshot).unwrap();

        let loaded_base = store.load().unwrap();
        let base_node_count = loaded_base.graph.node_count();

        let mut patch = Patch::new(1, 0);
        let file_patch = FilePatch {
            old_hash: "old".to_string(),
            new_hash: "new".to_string(),
            removed_node_ids: (0..nodes_before).map(|i| format!("node_{}", i)).collect(),
            added_nodes: (0..nodes_after).map(|i| SerializedNode {
                id: format!("node_{}", i + 100),
                category: "function".to_string(),
                kind: "function".to_string(),
                language: "rust".to_string(),
                name: format!("new_{}", i),
                path: Some("src/main.rs".to_string()),
                location: None,
                metadata: std::collections::HashMap::new(),
                description: None,
                features: vec![],
                feature_path: None,
                signature: None,
                source_ref: None,
                semantic_feature: None,
                node_level: "low".to_string(),
                documentation: None,
            }).collect(),
            removed_edges: Vec::new(),
            added_edges: Vec::new(),
        };
        patch.changes.modified_files.insert("src/main.rs".to_string(), file_patch);

        store.write_patch(&patch).unwrap();

        let loaded = store.load().unwrap();
        prop_assert_eq!(base_node_count, nodes_before);
        prop_assert_eq!(loaded.graph.node_count(), nodes_after);
    }
}
