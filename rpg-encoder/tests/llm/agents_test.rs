#![cfg(feature = "llm")]

use rpg_encoder::LlmConfig;
use rpg_encoder::{ExtractionScope, OrganizationMode, SemanticConfig};

#[test]
fn test_semantic_config_defaults() {
    let llm_config = LlmConfig::default();
    let config = SemanticConfig::new(llm_config);

    assert_eq!(config.scope, ExtractionScope::File);
    assert_eq!(config.organization, OrganizationMode::None);
}

#[test]
fn test_semantic_config_builders() {
    let llm_config = LlmConfig::default();
    let config = SemanticConfig::new(llm_config)
        .with_scope(ExtractionScope::Module)
        .with_organization(OrganizationMode::LlmBased);

    assert_eq!(config.scope, ExtractionScope::Module);
    assert_eq!(config.organization, OrganizationMode::LlmBased);
}

#[test]
fn test_organization_mode_default() {
    assert_eq!(OrganizationMode::default(), OrganizationMode::None);
}
