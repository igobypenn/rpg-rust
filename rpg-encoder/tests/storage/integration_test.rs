use rpg_encoder::{RpgEncoder, RpgSnapshot, RpgStore};
use tempfile::TempDir;

fn create_test_repo(dir: &std::path::Path) {
    let src = dir.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("main.rs"), "fn main() {}\n").unwrap();
}

#[test]
fn test_init_and_open_store() {
    let dir = TempDir::new().unwrap();
    create_test_repo(dir.path());

    let mut encoder = RpgEncoder::new().unwrap();
    let store = encoder.init_store(dir.path()).unwrap();
    assert_eq!(store.patch_count(), 0);

    let mut encoder2 = RpgEncoder::new().unwrap();
    let store2 = encoder2.open_store(dir.path()).unwrap();
    assert_eq!(store2.patch_count(), 0);
}

#[test]
fn test_encode_save_load_cycle() {
    let dir = TempDir::new().unwrap();
    create_test_repo(dir.path());
    let repo = dir.path().join("src");

    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(&repo).unwrap();
    assert!(result.graph.node_count() > 0);

    encoder.init_store(dir.path()).unwrap();

    let mut snapshot = RpgSnapshot::new("test", dir.path());
    snapshot.graph = result.graph;
    snapshot.compute_file_hashes().ok();

    encoder.store_mut().unwrap().save_base(&snapshot).unwrap();

    let loaded = RpgStore::open(dir.path()).unwrap().load().unwrap();
    assert_eq!(loaded.graph.node_count(), snapshot.graph.node_count());
}
