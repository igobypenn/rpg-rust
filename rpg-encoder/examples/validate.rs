use rpg_encoder::encoder::ValidationReport;
use rpg_encoder::RpgEncoder;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = if args.len() > 1 {
        Path::new(&args[1])
    } else {
        Path::new(".")
    };

    let mut encoder = RpgEncoder::new().unwrap();
    let result = match encoder.encode(path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let report = ValidationReport::from_graph(&result.graph);

    println!(
        "Nodes: {}  Edges: {}",
        report.total_nodes, report.total_edges
    );
    println!(
        "Import resolution: {:.1}%",
        report.import_resolution_rate * 100.0
    );
    println!(
        "Calls: {}  Implements: {}  FFI: {}",
        report.call_edge_count, report.implements_edge_count, report.ffi_edge_count
    );

    if !result.parse_errors.is_empty() {
        println!("Parse errors: {}", result.parse_errors.len());
    }
    for warning in &report.warnings {
        eprintln!("WARNING: {}", warning);
    }
}
