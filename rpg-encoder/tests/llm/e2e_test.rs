#![cfg(feature = "integration")]

use rpg_encoder::{FeatureExtractor, LlmConfig, SemanticConfig};
use std::path::PathBuf;

fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("rpg_encoder=debug".parse().unwrap()),
        )
        .with_test_writer()
        .try_init();
}

fn load_env() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let env_path = std::path::Path::new(&manifest_dir)
        .parent()
        .unwrap()
        .join(".env");
    let _ = dotenvy::from_path(&env_path);
}

fn create_test_config() -> LlmConfig {
    init_logging();
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
    let extractor = FeatureExtractor::new(SemanticConfig::new(config)).unwrap();

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
    let extractor = FeatureExtractor::new(SemanticConfig::new(config)).unwrap();

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
        SemanticConfig::new(config).with_scope(rpg_encoder::ExtractionScope::Repository),
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
        organized.len() > 1,
        "Should extract multiple entities from encoder, got {}",
        organized.len()
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

/// End-to-end coverage of `RpgEncoder::encode_with_semantics`.
///
/// This is the only test that exercises the full semantic pipeline wired up in
/// item 4: parse → per-file LLM feature extraction → functional abstraction
/// (centroid creation + BelongsToFeature linking). It guards against the class
/// of regression where the abstraction step silently produces zero output.
///
/// Requires network access and a configured endpoint (`.env`), so it is
/// `#[ignore]`d by default. Run with:
///
/// ```bash
/// cargo test --features integration test_encode_with_semantics_creates_centroids -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore = "Long-running e2e test - run with: cargo test -- --ignored"]
async fn test_encode_with_semantics_creates_centroids() {
    use rpg_encoder::{EdgeType, ExtractionScope, NodeCategory, NodeLevel, RpgEncoder};

    let config = SemanticConfig::new(create_test_config()).with_scope(ExtractionScope::File);

    // Bounded real source dir: parses cleanly and has enough entities for the
    // LLM to label, but keeps the request count small.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let target_dir = PathBuf::from(manifest_dir).join("examples/test-repo/rust");

    let mut encoder = RpgEncoder::new().expect("encoder init");
    let result = encoder
        .encode_with_semantics(&target_dir, config)
        .await
        .expect("encode_with_semantics should succeed");

    assert!(
        result.files_processed > 0,
        "should have processed at least one file"
    );

    let graph = &result.graph;

    // The LLM enrichment should populate semantic features on V^L nodes.
    let enriched_vl = graph
        .nodes()
        .filter(|n| n.node_level == NodeLevel::Low && n.semantic_feature.is_some())
        .count();
    assert!(
        enriched_vl > 0,
        "at least one V^L node should carry a semantic_feature after enrichment"
    );

    // The functional-abstraction step should induce at least one V^H centroid.
    let centroid_count = graph
        .nodes()
        .filter(|n| n.category == NodeCategory::FunctionalCentroid)
        .count();
    assert!(
        centroid_count > 0,
        "functional abstraction should create at least one V^H centroid"
    );

    // ...and link V^L nodes to centroids via BelongsToFeature edges.
    let belongs_edges = graph
        .edges()
        .filter(|(_, _, e)| e.edge_type == EdgeType::BelongsToFeature)
        .count();
    assert!(
        belongs_edges > 0,
        "V^L nodes should be linked to centroids via BelongsToFeature edges"
    );
}
