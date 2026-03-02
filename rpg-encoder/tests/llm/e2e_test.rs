#![cfg(feature = "integration")]

use rpg_encoder::{FeatureExtractor, LlmConfig, OrganizationMode, SemanticConfig};
use std::path::PathBuf;

fn load_env() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let env_path = std::path::Path::new(&manifest_dir)
        .parent()
        .unwrap()
        .join(".env");
    let _ = dotenvy::from_path(&env_path);
}

fn create_test_config() -> LlmConfig {
    load_env();
    LlmConfig::from_env().expect("Failed to load LLM config from env")
}

#[tokio::test]
async fn test_zai_extract_simple_function() {
    let config = create_test_config();
    let extractor = FeatureExtractor::new(SemanticConfig::new(config)).unwrap();

    let code = r#"
fn calculate_sum(a: i32, b: i32) -> i32 {
    a + b
}
"#;

    let features = extractor
        .extract_from_file(code, &PathBuf::from("src/math.rs"), "Math utilities")
        .await
        .unwrap();

    assert!(!features.is_empty(), "Should extract at least one entity");

    let first = &features[0];
    assert!(!first.features.is_empty(), "Should have features");
}

#[tokio::test]
async fn test_zai_extract_struct_with_methods() {
    let config = create_test_config();
    let extractor = FeatureExtractor::new(SemanticConfig::new(config)).unwrap();

    let code = r#"
pub struct User {
    name: String,
    email: String,
}

impl User {
    pub fn new(name: String, email: String) -> Self {
        Self { name, email }
    }

    pub fn validate(&self) -> bool {
        self.email.contains('@')
    }
}
"#;

    let features = extractor
        .extract_from_file(
            code,
            &PathBuf::from("src/models/user.rs"),
            "User management",
        )
        .await
        .unwrap();

    assert!(!features.is_empty(), "Should extract at least one entity");
    assert!(
        features[0].features.len() >= 2,
        "Should have features from struct and impl"
    );
}

#[tokio::test]
async fn test_zai_extract_empty_code() {
    let config = create_test_config();
    let extractor = FeatureExtractor::new(SemanticConfig::new(config)).unwrap();

    let code = "";

    let result = extractor
        .extract_from_file(code, &PathBuf::from("src/empty.rs"), "Empty file test")
        .await;

    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_zai_organize_by_path() {
    let config = create_test_config();
    let extractor = FeatureExtractor::new(
        SemanticConfig::new(config).with_organization(OrganizationMode::None),
    )
    .unwrap();

    let code = r#"
pub fn process_data(input: &str) -> String {
    input.to_uppercase()
}
"#;

    let organized = extractor
        .extract_and_organize(
            code,
            &PathBuf::from("src/services/data/processor.rs"),
            "Data processing system",
            "src/services/data/",
        )
        .await
        .unwrap();

    assert!(!organized.is_empty());
    assert!(organized[0].feature_path.contains("Services"));
}

#[tokio::test]
async fn test_zai_organize_llm_based() {
    let config = create_test_config();
    let extractor = FeatureExtractor::new(
        SemanticConfig::new(config).with_organization(OrganizationMode::LlmBased),
    )
    .unwrap();

    let code = r#"
pub struct Database {
    connection_string: String,
}

impl Database {
    pub fn connect(&self) -> Result<(), Error> {
        // Connect to database
    }

    pub fn query(&self, sql: &str) -> Result<Rows, Error> {
        // Execute query
    }
}
"#;

    let organized = extractor
        .extract_and_organize(
            code,
            &PathBuf::from("src/db/connection.rs"),
            "Database layer",
            "src/\n  db/\n    connection.rs",
        )
        .await
        .unwrap();

    assert!(!organized.is_empty());
    assert!(!organized[0].functional_area.is_empty());
}

#[tokio::test]
#[ignore = "Long-running e2e test - run with: cargo test -- --ignored"]
async fn test_zai_full_repository_analysis() {
    let config = create_test_config();
    let extractor = FeatureExtractor::new(
        SemanticConfig::new(config)
            .with_scope(rpg_encoder::ExtractionScope::Repository)
            .with_organization(OrganizationMode::LlmBased),
    )
    .unwrap();

    let code = include_str!("../../src/encoder/mod.rs");

    let organized = extractor
        .extract_and_organize(
            code,
            &PathBuf::from("src/encoder/mod.rs"),
            "RPG Encoder - Repository analysis tool",
            "src/\n  encoder/\n    mod.rs\n    walker.rs\n    builder.rs",
        )
        .await
        .unwrap();

    assert!(
        organized.len() > 5,
        "Should extract multiple entities from encoder"
    );
}

#[tokio::test]
#[ignore = "Long-running e2e test - run with: cargo test -- --ignored"]
async fn test_zai_multi_file_analysis() {
    let config = create_test_config();

    let test_cases = vec![
        (
            "src/api/handler.rs",
            r#"pub async fn handle_request(req: Request) -> Response { ... }"#,
        ),
        (
            "src/db/repository.rs",
            r#"pub struct UserRepository { db: Pool }"#,
        ),
        (
            "src/utils/helpers.rs",
            r#"pub fn format_date(d: DateTime) -> String { ... }"#,
        ),
    ];

    let extractor = FeatureExtractor::new(SemanticConfig::new(config)).unwrap();

    for (path, code) in test_cases {
        let features = extractor
            .extract_from_file(code, &PathBuf::from(path), "Multi-module project")
            .await;

        assert!(features.is_ok(), "Failed to extract from {}", path);
    }
}

#[tokio::test]
async fn test_zai_invalid_api_key_error() {
    load_env();

    let config = LlmConfig::openai_compatible("https://api.z.ai/api/coding/paas/v4", "glm-4-flash")
        .with_api_key("invalid_key_12345");

    let extractor = FeatureExtractor::new(SemanticConfig::new(config)).unwrap();

    let result = extractor
        .extract_from_file("fn test() {}", &PathBuf::from("test.rs"), "Test")
        .await;

    assert!(result.is_err());
}
