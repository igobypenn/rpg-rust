use rpg_encoder::{EdgeType, RpgEncoder};

fn repo_path(name: &str) -> Option<std::path::PathBuf> {
    let candidates = [
        format!("/home/ptasinga/src/{}", name),
        format!("../{}", name),
    ];
    for path in &candidates {
        let p = std::path::PathBuf::from(path);
        if p.is_dir() && p.read_dir().is_ok_and(|mut d| d.next().is_some()) {
            return Some(p);
        }
    }
    None
}

macro_rules! cross_repo_test {
    ($name:ident, $repo:expr, $block:expr) => {
        #[test]
        fn $name() {
            let path = repo_path($repo);
            if path.is_none() {
                eprintln!("Skipping {}: repo '{}' not found", stringify!($name), $repo);
                return;
            }
            let path = path.unwrap();
            $block(&path)
        }
    };
}

cross_repo_test!(llama_cpp_extends_edges, "llama.cpp", |path: &std::path::Path| {
    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(path).unwrap();

    let extends_count = result
        .graph
        .edges()
        .filter(|(_, _, e)| e.edge_type == EdgeType::Extends)
        .count();

    assert!(
        extends_count > 0,
        "llama.cpp should have Extends edges (C++ inheritance), got {}",
        extends_count
    );

    println!(
        "llama.cpp: {} nodes, {} edges, {} extends",
        result.graph.node_count(),
        result.graph.edge_count(),
        extends_count
    );
});

cross_repo_test!(llama_cpp_reduced_false_positives, "llama.cpp", |path: &std::path::Path| {
    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(path).unwrap();

    let call_edges: Vec<_> = result
        .graph
        .edges()
        .filter(|(_, _, e)| e.edge_type == EdgeType::Calls)
        .collect();

    let common_targets = ["tn", "cb", "clone", "unwrap", "text"];
    for target in &common_targets {
        let matches: Vec<_> = call_edges
            .iter()
            .filter(|(_, target_id, _)| {
                result
                    .graph
                    .get_node(*target_id)
                    .map(|n| n.name == *target)
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            matches.is_empty(),
            "Should not have Calls edge to filtered method '{}', found {} edges",
            target,
            matches.len()
        );
    }

    println!(
        "llama.cpp: {} call edges (no false positives to filtered methods)",
        call_edges.len()
    );
});

cross_repo_test!(opencode_self_encoding, "opencode", |path: &std::path::Path| {
    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(path).unwrap();

    let node_count = result.graph.node_count();
    let edge_count = result.graph.edge_count();

    assert!(
        node_count > 100,
        "opencode should have >100 nodes, got {}",
        node_count
    );
    assert!(
        edge_count > 100,
        "opencode should have >100 edges, got {}",
        edge_count
    );

    let impl_edges = result
        .graph
        .edges()
        .filter(|(_, _, e)| e.edge_type == EdgeType::Implements)
        .count();

    assert!(
        impl_edges > 0,
        "opencode should have Implements edges, got {}",
        impl_edges
    );

    println!(
        "opencode: {} nodes, {} edges, {} impl edges",
        node_count, edge_count, impl_edges
    );
});

cross_repo_test!(rpg_generator_validates, "rpg-generator", |path: &std::path::Path| {
    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(path).unwrap();

    let node_count = result.graph.node_count();
    let edge_count = result.graph.edge_count();

    assert!(
        node_count > 50,
        "rpg-generator should have >50 nodes, got {}",
        node_count
    );
    assert!(
        edge_count > 50,
        "rpg-generator should have >50 edges, got {}",
        edge_count
    );

    println!(
        "rpg-generator: {} nodes, {} edges",
        node_count, edge_count
    );
});
