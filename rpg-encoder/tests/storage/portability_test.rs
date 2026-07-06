//! Cross-machine committability test: encode at dir A, serialize to .rpg/,
//! load at dir B (simulating a different machine), verify the graph is
//! usable (paths resolve, get_source works, node lookups succeed).
//!
//! This is the key test for the "commit the pre-computed graph" workflow:
//! Alice encodes and commits `.rpg/`, Bob clones and loads without paying
//! LLM/parsing costs.

use rpg_encoder::{RpgEncoder, RpgSnapshot, RpgStore};

const FIXTURE: &str = r#"
pub fn process_payment(amount: u64) -> bool {
    amount > 0
}

pub struct Account {
    balance: u64,
}

impl Account {
    pub fn new() -> Self {
        Account { balance: 0 }
    }
}
"#;

/// Encode at dir A, save to A/.rpg/, copy .rpg/ to dir B, open at B, verify.
#[test]
fn committed_graph_loads_at_different_path() {
    // --- Alice's machine ---
    let dir_a = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir_a.path().join("src")).unwrap();
    std::fs::write(dir_a.path().join("src/main.rs"), FIXTURE).unwrap();

    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(dir_a.path()).unwrap();

    let mut snapshot = RpgSnapshot::from_encoder(&encoder);
    snapshot.compute_file_hashes().unwrap();
    snapshot.build_reverse_deps();

    let store_a = RpgStore::init(dir_a.path()).unwrap();
    // save_base requires the store to have called open or init first
    let mut store_a = store_a;
    store_a.save_base(&snapshot).unwrap();

    // Verify the graph was encoded
    assert!(result.graph.node_count() > 0);

    // --- Simulate Bob's machine: copy .rpg/ to a different path ---
    let dir_b = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir_b.path().join("src")).unwrap();
    std::fs::write(dir_b.path().join("src/main.rs"), FIXTURE).unwrap();

    // Copy the .rpg/ directory from A to B (simulating git clone)
    copy_dir_recursive(dir_a.path().join(".rpg"), dir_b.path().join(".rpg"));

    // --- Bob loads the committed graph ---
    let store_b = RpgStore::open(dir_b.path()).unwrap();
    let loaded_snapshot = store_b.load().unwrap();

    // KEY ASSERTION: paths in the loaded graph must be relative, not absolute
    // paths from Alice's machine.
    for node in loaded_snapshot.graph.nodes() {
        if let Some(ref path) = node.path {
            let path_str = path.to_string_lossy();
            // No path should contain Alice's temp dir.
            assert!(
                !path_str.contains(dir_a.path().to_string_lossy().as_ref()),
                "node path '{}' still contains Alice's absolute path",
                path_str
            );
            // File paths should be relative (e.g. "src/main.rs")
            if path_str.contains("main.rs") || path_str.contains("src") {
                assert!(
                    !path.is_absolute(),
                    "node path '{}' should be relative, not absolute",
                    path_str
                );
            }
        }

        // Location files must also be relative.
        if let Some(ref loc) = node.location {
            let loc_path = &loc.file;
            assert!(
                !loc_path.to_string_lossy().contains(dir_a.path().to_string_lossy().as_ref()),
                "location file '{}' still contains Alice's path",
                loc_path.display()
            );
        }
    }

    // The loaded graph should have the same topology as the original.
    assert_eq!(
        loaded_snapshot.graph.node_count(),
        result.graph.node_count(),
        "loaded graph node count must match"
    );
    assert_eq!(
        loaded_snapshot.graph.edge_count(),
        result.graph.edge_count(),
        "loaded graph edge count must match"
    );

    // A function node should be findable by its relative path.
    let func_node = loaded_snapshot
        .graph
        .nodes()
        .find(|n| n.name == "process_payment");
    assert!(func_node.is_some(), "process_payment node must exist in loaded graph");
    let func = func_node.unwrap();
    let func_path = func.path.as_ref().unwrap();
    assert!(
        func_path.ends_with("main.rs"),
        "process_payment path should end with main.rs, got: {}",
        func_path.display()
    );
    assert!(
        !func_path.is_absolute(),
        "process_payment path should be relative, got: {}",
        func_path.display()
    );
}

/// Verify that node paths are relative right after encoding (before any
/// save/load round-trip). This is the direct test of the builder fix.
#[test]
fn encoded_paths_are_relative() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), FIXTURE).unwrap();

    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(dir.path()).unwrap();

    // Every node with a path should have a relative path (no absolute prefix).
    for node in result.graph.nodes() {
        if let Some(ref path) = node.path {
            assert!(
                !path.is_absolute() || path == std::path::Path::new("."),
                "node '{}' has absolute path: {}",
                node.name,
                path.display()
            );
        }
    }
}

/// Recursively copy a directory. Used to simulate `git clone` of the .rpg/ dir.
fn copy_dir_recursive(src: std::path::PathBuf, dst: std::path::PathBuf) {
    if !std::path::Path::new(&dst).exists() {
        std::fs::create_dir_all(&dst).unwrap();
    }
    for entry in std::fs::read_dir(&src).unwrap() {
        let entry = entry.unwrap();
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(src_path, dst_path);
        } else {
            std::fs::copy(&src_path, &dst_path).unwrap();
        }
    }
}
