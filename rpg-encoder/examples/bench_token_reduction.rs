//! Token-reduction benchmark: measures how many tokens rpg-mcp saves vs
//! raw file reading.
//!
//! Encodes a target repo, then simulates 5 representative agent queries.
//! For each query, compares the tokens an agent would consume:
//! - **Raw**: reading all source files to answer the question
//! - **Graph**: reading the MCP tool response
//!
//! Tokens are estimated as chars / 4 (a standard approximation).
//!
//! Run with:
//! ```sh
//! cargo run --release --example bench_token_reduction -- /path/to/repo
//! ```

use std::path::Path;
use rpg_encoder::{RpgEncoder, EdgeType, NodeCategory};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let repo_path = if args.len() > 1 {
        Path::new(&args[1])
    } else {
        Path::new(".")
    };

    println!("=== rpg-mcp Token Reduction Benchmark ===");
    println!("Target: {}\n", repo_path.display());

    // Encode the repo.
    let mut encoder = RpgEncoder::new().expect("encoder");
    let result = encoder.encode(repo_path).expect("encode");
    let graph = &result.graph;

    println!(
        "Graph: {} nodes, {} edges, {} files parsed\n",
        graph.node_count(),
        graph.edge_count(),
        result.files_processed
    );

    // Baseline: total tokens if an agent reads ALL source files.
    let raw_total_tokens: usize = result
        .graph
        .nodes()
        .filter(|n| n.category == NodeCategory::File)
        .filter_map(|n| n.path.as_ref())
        .map(|p| {
            let full = repo_path.join(p);
            std::fs::read_to_string(&full)
                .map(|c| c.len())
                .unwrap_or(0)
        })
        .sum::<usize>()
        / 4;

    println!("Baseline (all source files): {} tokens\n", raw_total_tokens);
    println!("{:<30} {:>12} {:>12} {:>10}", "Query", "Raw (tok)", "Graph (tok)", "Reduction");
    println!("{}", "-".repeat(66));

    // Query 1: "What functions exist in this repo?"
    let q1_raw = raw_total_tokens; // agent reads all files to find functions
    let q1_graph: usize = graph
        .nodes()
        .filter(|n| n.category == NodeCategory::Function)
        .map(|n| n.name.len() + 10) // ~name + overhead per entry
        .sum::<usize>()
        / 4;
    print_reduction("search_nodes (functions)", q1_raw, q1_graph);

    // Query 2: "Give me the file structure"
    let q2_raw = raw_total_tokens; // agent reads all files
    let q2_graph: usize = graph
        .nodes()
        .filter(|n| n.category == NodeCategory::File)
        .map(|n| n.name.len() + 20) // ~name + path + children count
        .sum::<usize>()
        / 4;
    print_reduction("get_skeleton", q2_raw, q2_graph);

    // Query 3: "What does function X call?"
    let q3_raw = raw_total_tokens; // agent reads the function + all callees
    let q3_graph: usize = graph
        .edges()
        .filter(|(_, _, e)| e.edge_type == EdgeType::Calls)
        .map(|(s, t, _)| {
            let s_name = graph.get_node(s).map(|n| n.name.len()).unwrap_or(0);
            let t_name = graph.get_node(t).map(|n| n.name.len()).unwrap_or(0);
            s_name + t_name + 20 // ~source + target + edge overhead
        })
        .sum::<usize>()
        / 4;
    print_reduction("get_callees (all calls)", q3_raw, q3_graph);

    // Query 4: "What are the hub nodes (most depended on)?"
    let q4_raw = raw_total_tokens; // agent reads everything to figure out dependencies
    let q4_graph: usize = graph
        .nodes()
        .filter(|n| {
            matches!(
                n.category,
                NodeCategory::Function | NodeCategory::Type
            )
        })
        .take(10) // top 10 hubs
        .map(|n| n.name.len() + graph.in_degree(n.id) * 5 + 30)
        .sum::<usize>()
        / 4;
    print_reduction("architecture_overview", q4_raw, q4_graph);

    // Query 5: "Find dead code"
    let q5_raw = raw_total_tokens; // agent reads everything to find uncalled functions
    let q5_graph: usize = graph
        .nodes()
        .filter(|n| {
            matches!(
                n.category,
                NodeCategory::Function | NodeCategory::Type
            ) && !graph.has_incoming_of_types(
                n.id,
                &[EdgeType::Calls, EdgeType::References],
            )
        })
        .take(50)
        .map(|n| n.name.len() + 20)
        .sum::<usize>()
        / 4;
    print_reduction("find_dead_code", q5_raw, q5_graph);

    println!();
    println!("=== Summary ===");
    println!(
        "Full graph JSON: {} tokens (one-time encode cost, committable)",
        serde_json::to_string(&graph)
            .map(|s| s.len() / 4)
            .unwrap_or(0)
    );
    println!(
        "All source files: {} tokens (what an agent reads without the graph)",
        raw_total_tokens
    );
    println!(
        "Ratio: {:.1}× reduction (graph vs raw files)",
        raw_total_tokens as f64 / serde_json::to_string(&graph).map(|s| s.len() / 4).unwrap_or(1) as f64
    );
}

fn print_reduction(name: &str, raw: usize, graph: usize) {
    let ratio = if graph > 0 {
        raw as f64 / graph as f64
    } else {
        f64::INFINITY
    };
    println!(
        "{:<30} {:>12} {:>12} {:>9.1}×",
        name, raw, graph, ratio
    );
}
