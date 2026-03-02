mod fixtures;

use rpg_encoder::languages::RustParser;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    println!("Memory Profiling Benchmarks");
    println!("============================\n");
    println!("Run with: cargo bench --bench memory");
    println!("This binary runs memory profiling for key operations.\n");

    profile_parser_init();
    profile_graph_construction();
    profile_encode_small();
    profile_encode_medium();
    profile_semantic_enrichment();

    println!("\nMemory profiling complete.");
}

fn profile_parser_init() {
    println!("\n[Parser Initialization]");

    let _profiler = dhat::Profiler::builder().testing().build();

    let parser = RustParser::new().expect("Failed to create parser");

    println!("  Parser initialized successfully");
    drop(parser);
}

fn profile_graph_construction() {
    println!("\n[Graph Construction - 1000 nodes]");

    let _profiler = dhat::Profiler::builder().testing().build();

    let mut graph = rpg_encoder::RpgGraph::new();

    for i in 0..1000 {
        let node = rpg_encoder::Node::new(
            rpg_encoder::NodeId::new(i),
            rpg_encoder::NodeCategory::Function,
            "function",
            "rust",
            &format!("func_{}", i),
        )
        .with_path(std::path::PathBuf::from(format!("src/module_{:04}.rs", i)));
        graph.add_node(node);
    }

    for i in 0..999 {
        graph.add_edge(
            rpg_encoder::NodeId::new(i),
            rpg_encoder::NodeId::new(i + 1),
            rpg_encoder::Edge::new(rpg_encoder::EdgeType::Calls),
        );
    }

    println!("  Nodes: {}", graph.node_count());
    println!("  Edges: {}", graph.edge_count());
}

fn profile_encode_small() {
    println!("\n[Encode Small Repository - 10 files]");

    let fixture = fixtures::FixtureSet::generate("mem_small", fixtures::FixtureConfig::small())
        .expect("Failed to generate fixture");

    let _profiler = dhat::Profiler::builder().testing().build();

    let mut encoder = rpg_encoder::RpgEncoder::new().expect("Failed to create encoder");
    let result = encoder.encode(&fixture.base_path).expect("Encode failed");

    println!("  Files processed: {}", result.files_processed);
    println!("  Nodes created: {}", result.graph.node_count());
    println!("  Edges created: {}", result.graph.edge_count());

    drop(fixture);
}

fn profile_encode_medium() {
    println!("\n[Encode Medium Repository - 50 files]");

    let fixture = fixtures::FixtureSet::generate("mem_medium", fixtures::FixtureConfig::medium())
        .expect("Failed to generate fixture");

    let _profiler = dhat::Profiler::builder().testing().build();

    let mut encoder = rpg_encoder::RpgEncoder::new().expect("Failed to create encoder");
    let result = encoder.encode(&fixture.base_path).expect("Encode failed");

    println!("  Files processed: {}", result.files_processed);
    println!("  Nodes created: {}", result.graph.node_count());
    println!("  Edges created: {}", result.graph.edge_count());

    drop(fixture);
}

#[cfg(feature = "llm")]
fn profile_semantic_enrichment() {
    println!("\n[Semantic Enrichment - Skipped (implementation pending)]");
    // TODO: Implement when SemanticEnricher, MockEmbeddingClient types are available
}

#[cfg(not(feature = "llm"))]
fn profile_semantic_enrichment() {
    println!("\n[Semantic Enrichment - Skipped (feature not enabled)]");
    println!("  Enable with: cargo bench --bench memory --features llm");
}
