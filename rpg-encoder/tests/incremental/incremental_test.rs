
use std::path::PathBuf;

use rpg_encoder::{compute_hash, CachedUnit, RpgSnapshot, UnitType, SNAPSHOT_VERSION};

#[test]
fn test_compute_hash_consistency() {
    let content = "fn main() { println!(\"hello\"); }";
    let hash1 = compute_hash(content);
    let hash2 = compute_hash(content);
    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 64);
}

#[test]
fn test_compute_hash_different() {
    let hash1 = compute_hash("fn foo() {}");
    let hash2 = compute_hash("fn bar() {}");
    assert_ne!(hash1, hash2);
}

#[test]
fn test_snapshot_new() {
    let snapshot = RpgSnapshot::new("test-repo", PathBuf::from("/tmp/test").as_path());
    assert_eq!(snapshot.repo_name, "test-repo");
    assert_eq!(snapshot.version, SNAPSHOT_VERSION);
    assert_eq!(snapshot.graph.node_count(), 0);
}

#[test]
fn test_cached_unit_new() {
    let unit = CachedUnit::new(
        "test_function".to_string(),
        UnitType::Function,
        "abc123".to_string(),
        1,
        10,
    );

    assert_eq!(unit.name, "test_function");
    assert_eq!(unit.unit_type, UnitType::Function);
    assert_eq!(unit.content_hash, "abc123");
    assert_eq!(unit.start_line, 1);
    assert_eq!(unit.end_line, 10);
    assert!(unit.features.is_empty());
    assert!(unit.description.is_empty());
    assert!(unit.node_id.is_none());
}

#[test]
fn test_cached_unit_builders() {
    let unit = CachedUnit::new(
        "test".to_string(),
        UnitType::Function,
        "hash".to_string(),
        1,
        5,
    )
    .with_features(vec!["feature1".to_string(), "feature2".to_string()])
    .with_description("A test function".to_string());

    assert_eq!(unit.features.len(), 2);
    assert_eq!(unit.description, "A test function");
}

#[test]
fn test_snapshot_save_load() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let snapshot_path = temp_dir.path().join("test.rpg");

    let mut snapshot = RpgSnapshot::new("test-repo", PathBuf::from("/tmp/test").as_path());
    snapshot.repo_info = "A test repository".to_string();

    let unit = CachedUnit::new(
        "main".to_string(),
        UnitType::Function,
        compute_hash("fn main() {}"),
        1,
        1,
    );
    snapshot
        .unit_cache
        .insert(PathBuf::from("src/main.rs"), vec![unit]);

    snapshot.save(&snapshot_path).unwrap();
    assert!(snapshot_path.exists());

    let loaded = RpgSnapshot::load(&snapshot_path).unwrap();
    assert_eq!(loaded.repo_name, "test-repo");
    assert_eq!(loaded.repo_info, "A test repository");
    assert_eq!(loaded.unit_cache.len(), 1);
}

#[test]
fn test_snapshot_reverse_deps() {
    use rpg_encoder::{EdgeType, Node, NodeCategory, NodeId};

    let mut snapshot = RpgSnapshot::new("test", PathBuf::from("/tmp").as_path());

    let node1 = Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "function",
        "rust",
        "caller",
    );
    let node2 = Node::new(
        NodeId::new(0),
        NodeCategory::Function,
        "function",
        "rust",
        "callee",
    );

    let id1 = snapshot.graph.add_node(node1);
    let id2 = snapshot.graph.add_node(node2);
    snapshot.graph.add_typed_edge(id1, id2, EdgeType::Calls);

    snapshot.build_reverse_deps();

    let deps = snapshot.dependents_of(id2);
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0], id1);

    let no_deps = snapshot.dependents_of(id1);
    assert!(no_deps.is_empty());
}

#[test]
fn test_snapshot_stats() {
    let snapshot = RpgSnapshot::new("test", PathBuf::from("/tmp").as_path());
    let stats = snapshot.stats();

    assert_eq!(stats.version, SNAPSHOT_VERSION);
    assert_eq!(stats.node_count, 0);
    assert_eq!(stats.edge_count, 0);
}
