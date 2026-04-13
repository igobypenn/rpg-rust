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

const IDENTIFY_AREAS_PROMPT: &str = r#"You are an expert software architect. Identify the main functional areas of a repository.

## Constraints:
1. Output 1–8 functional areas (be conservative)
2. Areas must be mutually exclusive and collectively cover the repo
3. Avoid vague buckets: Core, Misc, Other, Utils
4. Use PascalCase names (e.g., "FeatureExtraction", "CodeParsing")
5. Do not include tests, docs, or build files as areas

## Output Format:
Return ONLY a JSON array. No markdown code blocks. No explanation.
["FunctionalArea1", "FunctionalArea2", ...]

Identify functional areas for this repository:

Repository: {repo_info}

Structure:
{skeleton}

Features:
{features_summary}"#;

const ASSIGN_FEATURES_PROMPT: &str = r#"You are an expert software architect. Assign features to functional areas and categories.

## Target Path Format:
{functional_area}/{category}/{subcategory}
- functional_area: Must be one of the provided areas
- category: Broader purpose (e.g., "parsing", "validation")
- subcategory: Specific context (e.g., "rust_parser", "token_check")

## Constraints:
1. Every feature MUST be assigned to exactly one path
2. Use only the provided functional areas
3. Paths should be meaningful and domain-aligned

## Output Format:
Return ONLY valid JSON. No markdown code blocks. No explanation.
{
  "functional_area/category/subcategory": ["feature_name_1", "feature_name_2"],
  ...
}

Assign these features to paths:

Functional Areas: {functional_areas}

Features:
{features}"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionScope {
    File,
    Module,
    Repository,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrganizationMode {
    #[default]
    None,
    LlmBased,
}

#[derive(Debug, Clone)]
pub struct SemanticConfig {
    pub llm: LlmConfig,
    pub scope: ExtractionScope,
    pub organization: OrganizationMode,
}

impl SemanticConfig {
    pub fn new(llm: LlmConfig) -> Self {
        Self {
            llm,
            scope: ExtractionScope::File,
            organization: OrganizationMode::None,
        }
    }

    pub fn with_scope(mut self, scope: ExtractionScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn with_organization(mut self, mode: OrganizationMode) -> Self {
        self.organization = mode;
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
    config: SemanticConfig,
}

impl FeatureExtractor {
    pub fn new(config: SemanticConfig) -> std::result::Result<Self, LlmError> {
        let client = Arc::new(OpenAIClient::new(config.llm.clone())?);
        Ok(Self { client, config })
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
        repo_skeleton: &str,
    ) -> std::result::Result<Vec<OrganizedFeature>, LlmError> {
        let features = self.extract_from_file(code, file_path, repo_info).await?;

        match self.config.organization {
            OrganizationMode::None => Ok(self.organize_by_path(&features, file_path)),
            OrganizationMode::LlmBased => {
                let organizer = ComponentOrganizer::new(self.client.clone());
                let functional_areas = organizer
                    .identify_functional_areas(repo_info, repo_skeleton, &features)
                    .await?;
                organizer
                    .organize_features(&features, &functional_areas)
                    .await
            }
        }
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

pub struct ComponentOrganizer {
    client: Arc<OpenAIClient>,
}

impl ComponentOrganizer {
    pub fn new(client: Arc<OpenAIClient>) -> Self {
        Self { client }
    }

    pub async fn identify_functional_areas(
        &self,
        repo_info: &str,
        repo_skeleton: &str,
        features: &[ExtractedFeature],
    ) -> std::result::Result<Vec<String>, LlmError> {
        let features_summary = features
            .iter()
            .map(|f| format!("{}: {:?}", f.entity_name, f.features))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = IDENTIFY_AREAS_PROMPT
            .replace("{repo_info}", repo_info)
            .replace("{skeleton}", repo_skeleton)
            .replace("{features_summary}", &features_summary);

        self.client.complete_json("", &prompt).await
    }

    pub async fn organize_features(
        &self,
        features: &[ExtractedFeature],
        functional_areas: &[String],
    ) -> std::result::Result<Vec<OrganizedFeature>, LlmError> {
        let features_str = features
            .iter()
            .map(|f| format!("{}: {:?}", f.entity_name, f.features))
            .collect::<Vec<_>>()
            .join("\n");

        let areas_str = functional_areas.join(", ");

        let prompt = ASSIGN_FEATURES_PROMPT
            .replace("{functional_areas}", &areas_str)
            .replace("{features}", &features_str);

        let assignments: std::collections::HashMap<String, Vec<String>> =
            self.client.complete_json("", &prompt).await?;

        let mut result = Vec::new();

        for feature in features {
            let (feature_path, functional_area) =
                self.find_assignment(&feature.entity_name, &assignments, functional_areas);

            result.push(OrganizedFeature {
                entity_name: feature.entity_name.clone(),
                features: feature.features.clone(),
                description: feature.description.clone(),
                feature_path,
                functional_area,
            });
        }

        Ok(result)
    }

    fn find_assignment(
        &self,
        entity_name: &str,
        assignments: &std::collections::HashMap<String, Vec<String>>,
        functional_areas: &[String],
    ) -> (String, String) {
        for (path, entities) in assignments {
            if entities.iter().any(|e| e == entity_name) {
                let functional_area = path.split('/').next().unwrap_or("Core").to_string();
                return (path.clone(), functional_area);
            }
        }

        let functional_area = functional_areas
            .first()
            .cloned()
            .unwrap_or_else(|| "Core".to_string());
        (
            format!("{}/General/{}", functional_area, entity_name),
            functional_area,
        )
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
