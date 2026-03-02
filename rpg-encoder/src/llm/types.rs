//! LLM response types with JSON Schema support for rig Extractor.
//!
//! These types implement `JsonSchema` from schemars to enable type-safe
//! structured output from LLM calls via rig's Extractor.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Response from feature extraction prompt.
///
/// Maps entity names (functions, structs, etc.) to their extracted features.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CodeAnalysis {
    /// Map of entity name -> extracted features
    pub entities: HashMap<String, EntityFeatures>,
}

/// Features extracted for a single code entity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EntityFeatures {
    /// List of semantic features in \"verb + object\" format
    /// (e.g., \"load config\", \"validate token\")
    pub features: Vec<String>,
    /// Brief description of the entity's purpose
    #[serde(default)]
    pub description: String,
    /// Semantic feature (f) - behavioral description from the paper.
    /// Describes WHAT the entity does functionally.
    /// Example: \"Handles user authentication and session management\"
    #[serde(default)]
    pub semantic_feature: String,
}

/// Response from functional area identification.
///
/// Contains a list of mutually exclusive functional areas that cover the repository.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FunctionalAreas {
    /// List of functional area names in PascalCase
    pub areas: Vec<String>,
}

/// Response from feature-to-area assignment.
///
/// Maps feature paths to lists of entity names assigned to that path.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FeatureAssignments {
    /// Map of feature path -> entity names
    pub assignments: HashMap<String, Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_analysis_deserialize() {
        let json = r#"{
            "entities": {
                "main": {
                    "features": ["entry point", "initialize app"],
                    "description": "Main entry point"
                }
            }
        }"#;

        let analysis: CodeAnalysis = serde_json::from_str(json).unwrap();
        assert_eq!(analysis.entities.len(), 1);
        assert_eq!(analysis.entities["main"].features.len(), 2);
    }

    #[test]
    fn test_functional_areas_deserialize() {
        let json = r#"{
            "areas": ["FeatureExtraction", "CodeParsing", "GraphConstruction"]
        }"#;

        let areas: FunctionalAreas = serde_json::from_str(json).unwrap();
        assert_eq!(areas.areas.len(), 3);
    }

    #[test]
    fn test_feature_assignments_deserialize() {
        let json = r#"{
            "assignments": {
                "FeatureExtraction/parsing/rust": ["extract_features", "parse_code"],
                "CodeParsing/core/tree_sitter": ["parse_file", "build_ast"]
            }
        }"#;

        let assignments: FeatureAssignments = serde_json::from_str(json).unwrap();
        assert_eq!(assignments.assignments.len(), 2);
    }
}
