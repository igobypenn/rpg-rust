//! Basic RPG generation example with opencode agent.
//!
//! This example demonstrates how to use the RPG generator to create
//! a codebase from a natural language description using the OpenCode agent.
//!
//! # Prerequisites
//!
//! Install opencode CLI: It should be in your PATH
//!
//! # Usage
//!
//! ```bash
//! # Run with opencode agent (no API key needed)
//! RUST_LOG=info cargo run --example basic_gen --features llm,opencode
//!
//! # Run with verbose logging
//! RUST_LOG=debug cargo run --example basic_gen --features llm,opencode
//!
//! # Run with all agents available
//! cargo run --example basic_gen --features llm,all-agents
//! ```

use rpg_generator::{GenerationRequest, RpgGenerator, TargetLanguage};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing to see detailed logs
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .with_thread_ids(false)
        .init();

    println!("=== RPG Generator with OpenCode Agent ===\n");

    // With opencode feature enabled, no LLM config is needed
    // The agent handles all phases (feature extraction, architecture, code generation)
    let output_dir = PathBuf::from("./generated-output");

    let generator = RpgGenerator::new()
        .with_output_dir(&output_dir)
        .with_max_test_iterations(5); // TDD loop iterations

    // Define what to generate
    let request = GenerationRequest::new(
        "A simple CLI todo list application with add, list, complete, and delete commands. \
         Store todos in a JSON file. Include proper error handling and unit tests.",
        TargetLanguage::Rust,
    );

    println!("Description: {}", request.description);
    println!("Language: {:?}", request.language);
    println!("Output directory: {}\n", output_dir.display());

    // Run the generation pipeline
    println!("Starting generation...\n");
    println!("Using OpenCode agent for all phases:");
    println!("  Phase 1: Feature extraction");
    println!("  Phase 2: Architecture design");
    println!("  Phase 3: Code generation with TDD loop\n");

    match generator.generate(request).await {
        Ok(output) => {
            println!("\n=== Generation Complete ===");
            println!("Request ID: {}", output.request.id);
            println!("Total files: {}", output.total_files());
            println!("Completed tasks: {}", output.completed_tasks());
            println!("Failed tasks: {}", output.failed_tasks());
            println!("Success rate: {:.0}%", output.success_rate() * 100.0);

            // Show generated files
            println!("\n--- Generated Files ---");
            for (path, content) in &output.result.generated_code {
                println!("  {} ({} bytes)", path.display(), content.len());
            }

            // Show task outcomes
            println!("\n--- Task Outcomes ---");
            for (task_id, outcome) in &output.result.task_outcomes {
                let status = match outcome.status {
                    rpg_generator::TaskStatus::Completed => "DONE",
                    rpg_generator::TaskStatus::Failed => "FAIL",
                    rpg_generator::TaskStatus::InProgress => "RUN",
                    _ => "WAIT",
                };
                println!(
                    "  {} {} ({} iterations, {:.0}% pass rate)",
                    status,
                    task_id,
                    outcome.iterations,
                    outcome.test_pass_rate * 100.0
                );
            }

            // Show test results if any
            if !output.result.test_results.is_empty() {
                println!("\n--- Test Results ---");
                for test_result in &output.result.test_results {
                    println!(
                        "  Task {}: {} passed, {} failed",
                        test_result.task_id, test_result.passed, test_result.failed
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("\n=== Generation Failed ===");
            eprintln!("Error: {}", e);

            // Provide helpful hints
            if e.to_string().contains("not found") || e.to_string().contains("not available") {
                eprintln!("\nHint: Make sure the opencode CLI is installed and in your PATH.");
                eprintln!("      You can check with: which opencode");
            }
        }
    }

    Ok(())
}
