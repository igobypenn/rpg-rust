use rpg_encoder::{compute_hash, CachedUnit, RpgSnapshot, UnitType, SNAPSHOT_VERSION};
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_snapshot_load_corrupted_json() {
    let temp_dir = TempDir::new().unwrap();
    let snapshot_path = temp_dir.path().join("corrupted.rpg");

    std::fs::write(&snapshot_path, "not valid json {{{").unwrap();

    let result = RpgSnapshot::load(&snapshot_path);
    assert!(result.is_err());
}

#[test]
fn test_snapshot_load_nonexistent_file() {
    let result = RpgSnapshot::load(&PathBuf::from("/nonexistent/path/snapshot.rpg"));
    assert!(result.is_err());
}

#[test]
fn test_snapshot_save_invalid_path() {
    let snapshot = RpgSnapshot::new("test", PathBuf::from("/tmp").as_path());
    let result = snapshot.save(&PathBuf::from("/nonexistent/directory/snapshot.rpg"));

    assert!(result.is_err());
}

#[test]
fn test_snapshot_large_unit_cache() {
    let mut snapshot = RpgSnapshot::new("test", PathBuf::from("/tmp").as_path());

    for i in 0..100 {
        let unit = CachedUnit::new(
            format!("func_{}", i),
            UnitType::Function,
            compute_hash(&format!("fn func_{}() {{}}", i)),
            i,
            i + 1,
        );
        let path = PathBuf::from(format!("src/file_{}.rs", i % 10));
        snapshot.unit_cache.entry(path).or_default().push(unit);
    }

    assert_eq!(snapshot.unit_cache.len(), 10);

    let total_units: usize = snapshot.unit_cache.values().map(|v| v.len()).sum();
    assert_eq!(total_units, 100);
}

#[test]
fn test_snapshot_version() {
    let snapshot = RpgSnapshot::new("test", PathBuf::from("/tmp").as_path());
    assert_eq!(snapshot.version, SNAPSHOT_VERSION);
}

#[test]
fn test_snapshot_save_load_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let snapshot_path = temp_dir.path().join("roundtrip.rpg");

    let mut original = RpgSnapshot::new("test-repo", PathBuf::from("/project").as_path());
    original.repo_info = "Test repository info".to_string();

    let unit = CachedUnit::new(
        "test_func".to_string(),
        UnitType::Function,
        compute_hash("fn test_func() {}"),
        10,
        15,
    )
    .with_features(vec!["feature1".to_string(), "feature2".to_string()])
    .with_description("A test function".to_string());

    original
        .unit_cache
        .insert(PathBuf::from("src/lib.rs"), vec![unit]);

    original.save(&snapshot_path).unwrap();
    assert!(snapshot_path.exists());

    let loaded = RpgSnapshot::load(&snapshot_path).unwrap();

    assert_eq!(loaded.repo_name, original.repo_name);
    assert_eq!(loaded.repo_info, original.repo_info);
    assert_eq!(loaded.unit_cache.len(), 1);
    assert_eq!(loaded.version, SNAPSHOT_VERSION);
}

#[test]
fn test_snapshot_empty_unit_cache() {
    let snapshot = RpgSnapshot::new("empty", PathBuf::from("/tmp").as_path());

    assert!(snapshot.unit_cache.is_empty());

    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("empty.rpg");

    snapshot.save(&path).unwrap();
    let loaded = RpgSnapshot::load(&path).unwrap();

    assert!(loaded.unit_cache.is_empty());
}

#[test]
fn test_cached_unit_with_node_id() {
    use rpg_encoder::NodeId;

    let unit = CachedUnit::new(
        "test".to_string(),
        UnitType::Function,
        "hash123".to_string(),
        1,
        5,
    )
    .with_node_id(NodeId::new(42));

    assert!(unit.node_id.is_some());
    assert_eq!(unit.node_id.unwrap(), NodeId::new(42));
}

#[test]
fn test_snapshot_timestamp_update() {
    let mut snapshot = RpgSnapshot::new("test", PathBuf::from("/tmp").as_path());

    // Update timestamp should work without error
    snapshot.update_timestamp();

    // Verify timestamp is reasonable (within last 10 seconds)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(snapshot.last_modified > 0);
    assert!(snapshot.last_modified <= now);
    assert!(snapshot.last_modified > now - 10);
}
