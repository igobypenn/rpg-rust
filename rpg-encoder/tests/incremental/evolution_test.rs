use rpg_encoder::{EvolutionSummary, RpgSnapshot};
use std::path::PathBuf;

#[test]
fn test_evolution_summary_default() {
    let summary = EvolutionSummary::default();

    assert_eq!(summary.files_added, 0);
    assert_eq!(summary.files_deleted, 0);
    assert_eq!(summary.files_modified, 0);
    assert_eq!(summary.units_added, 0);
    assert_eq!(summary.units_changed, 0);
    assert_eq!(summary.units_deleted, 0);
    assert_eq!(summary.nodes_created, 0);
    assert_eq!(summary.nodes_removed, 0);
    assert_eq!(summary.nodes_updated, 0);
    assert_eq!(summary.edges_rebuilt, 0);
    assert_eq!(summary.llm_calls, 0);
    assert_eq!(summary.cache_hits, 0);
}

#[test]
fn test_evolution_summary_fields() {
    let mut summary = EvolutionSummary::default();
    summary.files_added = 5;
    summary.files_deleted = 2;
    summary.files_modified = 3;
    summary.units_added = 10;
    summary.cache_hits = 8;

    assert_eq!(summary.files_added, 5);
    assert_eq!(summary.files_deleted, 2);
    assert_eq!(summary.units_added, 10);
    assert_eq!(summary.cache_hits, 8);
}

#[test]
fn test_snapshot_for_evolution() {
    let snapshot = RpgSnapshot::new("test-repo", PathBuf::from("/tmp/test").as_path());

    assert_eq!(snapshot.repo_name, "test-repo");
    assert_eq!(snapshot.graph.node_count(), 0);
}

#[test]
fn test_snapshot_graph_manipulation() {
    use rpg_encoder::{EdgeType, Node, NodeCategory, NodeId};

    let mut snapshot = RpgSnapshot::new("test", PathBuf::from("/tmp").as_path());

    let n1 = snapshot.graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "func_a",
    ));
    let n2 = snapshot.graph.add_node(Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "fn",
        "rust",
        "func_b",
    ));

    snapshot.graph.add_typed_edge(n1, n2, EdgeType::Calls);
    snapshot.build_reverse_deps();

    assert_eq!(snapshot.graph.node_count(), 2);
    assert_eq!(snapshot.graph.edge_count(), 1);

    let deps = snapshot.dependents_of(n2);
    assert_eq!(deps.len(), 1);
}

#[test]
fn test_evolution_summary_clone() {
    let mut summary = EvolutionSummary::default();
    summary.files_added = 3;
    summary.cache_hits = 7;

    let cloned = summary.clone();

    assert_eq!(cloned.files_added, 3);
    assert_eq!(cloned.cache_hits, 7);
}

#[test]
fn test_snapshot_with_files() {
    use rpg_encoder::{compute_hash, CachedUnit};

    let mut snapshot = RpgSnapshot::new("test", PathBuf::from("/tmp").as_path());

    let unit = CachedUnit::new(
        "main".to_string(),
        rpg_encoder::UnitType::Function,
        compute_hash("fn main() {}"),
        1,
        1,
    );

    snapshot
        .unit_cache
        .insert(PathBuf::from("src/main.rs"), vec![unit]);

    assert_eq!(snapshot.unit_cache.len(), 1);
    assert!(snapshot
        .unit_cache
        .contains_key(&PathBuf::from("src/main.rs")));
}

#[test]
fn test_evolution_empty_diff() {
    let snapshot = RpgSnapshot::new("test", PathBuf::from("/tmp").as_path());

    assert_eq!(snapshot.graph.node_count(), 0);
    assert!(snapshot.unit_cache.is_empty());
}
