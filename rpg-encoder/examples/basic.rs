use std::path::Path;

use rpg_encoder::RpgEncoder;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let path = if args.len() > 1 {
        Path::new(&args[1])
    } else {
        Path::new(".")
    };

    println!("Encoding repository: {}", path.display());

    let mut encoder = match RpgEncoder::new() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to initialize encoder: {}", e);
            std::process::exit(1);
        }
    };

    match encoder.encode(path) {
        Ok(result) => {
            println!("Successfully encoded repository!");
            println!("  Nodes: {}", result.graph.node_count());
            println!("  Edges: {}", result.graph.edge_count());
            println!("  Files processed: {}", result.files_processed);
            println!("  Files skipped: {}", result.files_skipped);
            if !result.parse_errors.is_empty() {
                println!("  Parse errors: {}", result.parse_errors.len());
            }

            match encoder.to_json() {
                Ok(json) => {
                    println!("\n--- JSON Output ---");
                    println!("{}", json);
                }
                Err(e) => {
                    eprintln!("Failed to serialize to JSON: {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to encode repository: {}", e);
            std::process::exit(1);
        }
    }
}
