#![cfg(feature = "llm")]

use rpg_encoder::{LlmConfig, LlmProvider};

#[test]
fn test_llm_config_default() {
    let config = LlmConfig::default();
    assert_eq!(config.provider, LlmProvider::OpenAI);
    assert_eq!(config.model, "gpt-4o-mini");
    assert_eq!(config.max_tokens, 4096);
}

#[test]
fn test_llm_config_openai_compatible() {
    let config =
        LlmConfig::openai_compatible("https://open.bigmodel.cn/api/paas/v4", "glm-4-flash");
    assert_eq!(config.provider, LlmProvider::OpenAICompatible);
    assert_eq!(config.model, "glm-4-flash");
    assert_eq!(
        config.base_url,
        Some("https://open.bigmodel.cn/api/paas/v4".to_string())
    );
}

#[test]
fn test_llm_config_builder() {
    let config = LlmConfig::default()
        .with_api_key("test-key")
        .with_max_tokens(2048)
        .with_temperature(0.5);

    assert_eq!(config.api_key, Some("test-key".to_string()));
    assert_eq!(config.max_tokens, 2048);
    assert!((config.temperature - 0.5).abs() < f32::EPSILON);
}

#[test]
fn test_llm_config_from_env_missing_key() {
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("OPENAI_BASE_URL");
    std::env::remove_var("OPENAI_MODEL");

    let result = LlmConfig::from_env();
    assert!(result.is_ok());
    let config = result.unwrap();
    assert!(config.api_key.is_none());
}
