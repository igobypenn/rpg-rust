use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

use crate::llm::{LlmConfig, LlmError, OpenAIClient};
use crate::utils::to_pascal_case;

const FEATURE_EXTRACTION_PROMPT: &str = r#"You are a senior software analyst, tasked with extracting high-level semantic features from code.

## Key Goals:
- Complete analysis: Include ALL functions, methods, types, and their responsibilities
- Focus on purpose and high-level behavior, not implementation details

## Feature Extraction Principles:
1. Use "verb + object" format (e.g., "load config", "validate token")
2. Use lowercase English only
3. 3–8 words per feature
4. Avoid vague verbs: "handle", "process", "deal with"
5. Avoid implementation details (loops, conditionals, data structures)
6. Prefer domain semantics over technical terms
7. Each feature should express one single responsibility

## Output Format:
Return ONLY valid JSON. No markdown code blocks. No explanation.
{
  "entities": {
    "entity_name": {
      "features": ["feature one", "feature two"],
      "description": "brief description"
    }
  }
}

Analyze this code:

Repository: {repo_info}
File: {file_path}

Code:
{code}"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionScope {
    File,
    Module,
    Repository,
}

#[derive(Debug, Clone)]
pub struct SemanticConfig {
    pub llm: LlmConfig,
    pub scope: ExtractionScope,
}

impl SemanticConfig {
    pub fn new(llm: LlmConfig) -> Self {
        Self {
            llm,
            scope: ExtractionScope::File,
        }
    }

    pub fn with_scope(mut self, scope: ExtractionScope) -> Self {
        self.scope = scope;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedFeature {
    pub entity_name: String,
    pub features: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizedFeature {
    pub entity_name: String,
    pub features: Vec<String>,
    pub description: String,
    pub feature_path: String,
    pub functional_area: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeAnalysis {
    pub entities: std::collections::HashMap<String, EntityFeatures>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityFeatures {
    pub features: Vec<String>,
    #[serde(default)]
    pub description: String,
}

pub struct FeatureExtractor {
    client: Arc<OpenAIClient>,
}

impl FeatureExtractor {
    pub fn new(_config: SemanticConfig) -> std::result::Result<Self, LlmError> {
        let client = Arc::new(OpenAIClient::new(_config.llm.clone())?);
        Ok(Self { client })
    }

    /// Get a reference to the underlying LLM client.
    pub fn client(&self) -> &OpenAIClient {
        &self.client
    }

    pub async fn extract_from_file(
        &self,
        code: &str,
        file_path: &Path,
        repo_info: &str,
    ) -> std::result::Result<Vec<ExtractedFeature>, LlmError> {
        let prompt = FEATURE_EXTRACTION_PROMPT
            .replace("{repo_info}", repo_info)
            .replace("{file_path}", file_path.to_string_lossy().as_ref())
            .replace("{code}", code);

        let analysis: CodeAnalysis = self.client.complete_json("", &prompt).await?;

        let features: Vec<ExtractedFeature> = analysis
            .entities
            .into_iter()
            .map(|(name, ef)| ExtractedFeature {
                entity_name: name,
                features: ef.features,
                description: ef.description,
            })
            .collect();

        Ok(features)
    }

    pub async fn extract_and_organize(
        &self,
        code: &str,
        file_path: &Path,
        repo_info: &str,
        _repo_skeleton: &str,
    ) -> std::result::Result<Vec<OrganizedFeature>, LlmError> {
        let features = self.extract_from_file(code, file_path, repo_info).await?;
        Ok(self.organize_by_path(&features, file_path))
    }

    pub fn organize_by_path(
        &self,
        features: &[ExtractedFeature],
        file_path: &Path,
    ) -> Vec<OrganizedFeature> {
        let base_path = file_path
            .with_extension("")
            .to_string_lossy()
            .replace(['/', '\\'], "/")
            .trim_start_matches("./")
            .trim_start_matches("src/")
            .to_string();

        features
            .iter()
            .map(|f| {
                let parts: Vec<&str> = base_path.split('/').take(3).collect();
                let functional_area = parts.first().unwrap_or(&"core").to_string();
                let category = parts.get(1).unwrap_or(&"general").to_string();
                let subcategory = parts.get(2).unwrap_or(&"misc").to_string();

                let feature_path = format!(
                    "{}/{}/{}",
                    to_pascal_case(&functional_area),
                    to_pascal_case(&category),
                    to_pascal_case(&subcategory)
                );

                OrganizedFeature {
                    entity_name: f.entity_name.clone(),
                    features: f.features.clone(),
                    description: f.description.clone(),
                    feature_path: format!("{}/{}", feature_path, f.entity_name),
                    functional_area: to_pascal_case(&functional_area),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn make_extractor() -> FeatureExtractor {
        let config = SemanticConfig::new(LlmConfig::openai_compatible(
            "http://localhost:11434/v1",
            "test-model",
        ));
        FeatureExtractor::new(config).expect("failed to create extractor")
    }

    #[test]
    fn test_organize_by_path_groups_by_functional_area() {
        let extractor = make_extractor();
        let features = vec![
            ExtractedFeature {
                entity_name: "Parser".to_string(),
                features: vec!["parse code".to_string()],
                description: "parses source".to_string(),
            },
            ExtractedFeature {
                entity_name: "Validator".to_string(),
                features: vec!["validate input".to_string()],
                description: "validates data".to_string(),
            },
        ];

        let file_path = Path::new("src/parsing/parser.rs");
        let organized = extractor.organize_by_path(&features, file_path);

        assert_eq!(organized.len(), 2);

        for of in &organized {
            assert_eq!(of.functional_area, "Parsing");
            assert!(
                of.feature_path.starts_with("Parsing/"),
                "feature_path should start with Parsing/ but got: {}",
                of.feature_path
            );
        }

        assert_eq!(organized[0].entity_name, "Parser");
        assert_eq!(organized[1].entity_name, "Validator");
    }

    #[test]
    fn test_organize_by_path_empty_features() {
        let extractor = make_extractor();
        let features: Vec<ExtractedFeature> = vec![];

        let file_path = Path::new("src/main.rs");
        let organized = extractor.organize_by_path(&features, file_path);

        assert!(organized.is_empty());
    }
}
