//! Focused LLM debug test with local llama.cpp endpoint
//!
//! Usage:
//!   cargo run --features llm --example llm_debug
//!
//! This tests the local llama.cpp LLM endpoint with minimal code for quick feedback.

use rpg_encoder::{LlmConfig, OpenAIClient};

fn load_env() {
    if let Ok(path) = std::env::current_dir() {
        let local_env = path.join(".env");
        let parent_env = path.parent().map(|p| p.join(".env"));

        if local_env.exists() {
            let _ = dotenvy::from_path(&local_env);
        } else if let Some(parent) = parent_env {
            if parent.exists() {
                let _ = dotenvy::from_path(&parent);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    load_env();

    println!("=== LLM Debug Test ===");
    println!();

    let config = match LlmConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load LLM config: {}", e);
            eprintln!();
            eprintln!("Set in .env or environment:");
            eprintln!("  OPENAI_API_KEY=your_key");
            eprintln!("  OPENAI_BASE_URL=http://localhost:8080/v1");
            eprintln!("  OPENAI_MODEL=model_name");
            std::process::exit(1);
        }
    };

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.openai.com/v1");
    let api_key = config.api_key.as_deref().unwrap_or("");
    println!("Configuration:");
    println!("  Endpoint: {}", base_url);
    println!("  Model: {}", config.model);
    println!("  API Key: {} chars", api_key.len());
    println!("  Max tokens: {}", config.max_tokens);
    println!("  Temperature: {}", config.temperature);
    println!();

    let client = match OpenAIClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {}", e);
            std::process::exit(1);
        }
    };

    println!("=== Testing LLM completion ===");
    let code = "fn add(a: i32, b: i32) -> i32 { a + b }";
    println!("Code: {}", code);
    println!();

    let system = "You are a code analyzer. Describe what this function does in one sentence.";
    let user = code;

    match client.complete(system, user).await {
        Ok(response) => {
            println!("=== LLM Response ===");
            println!("{}", response);
            println!();
            println!("=== SUCCESS ===");
        }
        Err(e) => {
            eprintln!("=== ERROR ===");
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}
