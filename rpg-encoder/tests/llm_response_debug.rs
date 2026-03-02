//! LLM Response Debug Test

use rpg_encoder::{LlmConfig, OpenAIClient};
use std::path::Path;

fn load_env() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let env_path = Path::new(&manifest_dir).parent().unwrap().join(".env");
    let _ = dotenvy::from_path(&env_path);
}

#[allow(dead_code)]
fn print_header() {
    println!();
    println!("=== LLM Response Debug Tests ===");
    println!("These tests show raw LLM responses to diagnose JSON parsing issues.");
    println!("Run with: cargo test --features llm --test llm_response_debug -- --nocapture");
    println!();
}

#[tokio::test]
async fn test_llm_raw_response_simple() {
    load_env();

    let config = LlmConfig::from_env().expect("Failed to load LLM config");
    let client = OpenAIClient::new(config).expect("Failed to create client");

    println!();
    println!("=== Test 1: Simple Code Description ===");
    println!("This tests a basic non-JSON request.\n");

    let system = "You are a code analyzer. Respond with a brief description.";
    let user = "fn add(a: i32, b: i32) -> i32 { a + b }";

    match client.complete(system, user).await {
        Ok(response) => {
            println!("Raw response:");
            println!("{}", response);
            println!();
            println!("--- End of response ---");
        }
        Err(e) => {
            eprintln!("ERROR: {}", e);
            panic!("LLM request failed");
        }
    }
}

#[tokio::test]
async fn test_llm_raw_response_json_request() {
    load_env();

    let config = LlmConfig::from_env().expect("Failed to load LLM config");
    let client = OpenAIClient::new(config).expect("Failed to create client");

    println!();
    println!("=== Test 2: JSON Format Request ===");
    println!("This tests JSON request and shows the extraction logic.\n");

    let system = r#"You are a code analyzer. You MUST respond ONLY with valid JSON, no other text.
This response must be a JSON object with this exact structure:
{
  "name": "function_name",
  "description": "brief description"
}

Do not include any text before or after the JSON. Do not use markdown code blocks.
Do not show your thinking process. Output ONLY the JSON object."#;

    let user = "fn add(a: i32, b: i32) -> i32 { a + b }";

    match client.complete(system, user).await {
        Ok(response) => {
            println!("Raw response:");
            println!("{}", response);
            println!();
            println!("--- Response analysis ---");
            println!("Length: {} chars", response.len());
            println!("Starts with '{{': {}", response.trim().starts_with('{'));
            println!("Ends with '}}': {}", response.trim().ends_with('}'));

            println!();

            // Check if valid JSON
            match serde_json::from_str::<serde_json::Value>(response.trim()) {
                Ok(json) => {
                    println!();
                    println!("✅ Response is valid JSON");
                    println!("{}", serde_json::to_string_pretty(&json).unwrap());
                }
                Err(_e) => {
                    println!();
                    println!("❌ Not valid JSON - checking for markdown blocks...");
                    let trimmed = response.trim();

                    // Try to extract from markdown code blocks
                    if trimmed.contains("```json") {
                        let start = trimmed.find("```json").unwrap();
                        if let Some(end) = trimmed.rfind("```") {
                            let json_str = &trimmed[start + 7..end];
                            println!();
                            println!("Extracted from ```json block:");
                            println!("{}", json_str);
                            match serde_json::from_str::<serde_json::Value>(json_str) {
                                Ok(json) => {
                                    println!();
                                    println!("✅ Extracted JSON is valid!");
                                    println!("{}", serde_json::to_string_pretty(&json).unwrap());
                                }
                                Err(e2) => {
                                    println!();
                                    println!("❌ Extracted JSON also invalid: {}", e2);
                                }
                            }
                        }
                    } else if trimmed.contains("```") {
                        let start = trimmed.find("```").unwrap();
                        if let Some(end) = trimmed[start..].rfind("```") {
                            let json_str = &trimmed[start + 3..end];
                            println!();
                            println!("Extracted from ``` block:");
                            println!("{}", json_str);
                            match serde_json::from_str::<serde_json::Value>(json_str) {
                                Ok(_) => {
                                    println!();
                                    println!("✅ Extracted JSON is valid!");
                                }
                                Err(e2) => {
                                    println!();
                                    println!("❌ Extracted JSON also invalid: {}", e2);
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("ERROR: {}", e);
            panic!("LLM request failed");
        }
    }
}

#[tokio::test]
async fn test_llm_json_extraction_logic() {
    load_env();

    let config = LlmConfig::from_env().expect("Failed to load LLM config");
    let _client = OpenAIClient::new(config).expect("Failed to create client");

    println!();
    println!("=== Test 3: JSON Extraction Logic Tests ===");
    println!("This tests the JSON extraction logic with simulated responses.\n");

    // Test the extract_json logic with different response formats
    let test_cases = vec![
        ("Plain JSON", r#"{"name": "add", "description": "test"}"#),
        (
            "Markdown json block",
            r#"```json
{"name": "add", "description": "test"}
```"#,
        ),
        (
            "With preamble text",
            r#"Here is the analysis:
```json
{"name": "add", "description": "test"}
```
Hope this helps!"#,
        ),
        (
            "Plain code block",
            r#"```
{"name": "add", "description": "test"}
```"#,
        ),
    ];

    for (name, response) in test_cases {
        println!("Testing: {}", name);
        println!("Input:");
        println!("{}", response);

        println!();

        // Simulate the extract_json logic from client.rs
        let extracted = {
            let start = response.find("```json").or_else(|| response.find("```"));
            let end = response.rfind("```");

            match (start, end) {
                (Some(s), Some(e)) if s < e => {
                    let json_start = response[s..].find('\n').map(|i| s + i + 1).unwrap_or(s + 7);
                    response[json_start..e].trim()
                }
                _ => response.trim(),
            }
        };

        println!("Extracted:");
        println!("{}", extracted);

        match serde_json::from_str::<serde_json::Value>(extracted) {
            Ok(json) => println!("✅ Parsed: {}", json),
            Err(e) => println!("❌ Parse error: {}", e),
        }
        println!("---");
    }
}
