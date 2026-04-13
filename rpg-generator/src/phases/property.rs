//! Phase 1: Property Level - Feature extraction and component planning.

use serde::{Deserialize, Serialize};

use crate::contract::ContractVerifier;
use crate::error::Result;
use crate::llm::{LlmClient, OpenAIClient};
use crate::{ComponentPlan, FeatureTree, GenerationPlan, GenerationRequest};

pub struct PropertyLevelBuilder {
    client: OpenAIClient,
    verifier: ContractVerifier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FeatureExtractionResponse {
    root_name: String,
    categories: Vec<CategoryResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CategoryResponse {
    name: String,
    description: Option<String>,
    features: Vec<String>,
    subcategories: Vec<SubcategoryResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubcategoryResponse {
    name: String,
    features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RefactoringResponse {
    components: Vec<ComponentResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ComponentResponse {
    name: String,
    description: String,
    features: Vec<String>,
}

impl PropertyLevelBuilder {
    pub fn new(client: OpenAIClient) -> Self {
        Self {
            client,
            verifier: ContractVerifier::new(),
        }
    }

    pub fn with_verifier(mut self, verifier: ContractVerifier) -> Self {
        self.verifier = verifier;
        self
    }

    pub async fn build(&self, request: &GenerationRequest) -> Result<GenerationPlan> {
        let feature_tree = self.extract_features(&request.description).await?;
        let component_plan = self.refactor_features(&feature_tree).await?;

        let mut plan = GenerationPlan::new(request.clone(), feature_tree, component_plan);
        plan.complete();

        self.verifier.verify_generation_plan(&plan)?;

        Ok(plan)
    }

    async fn extract_features(&self, description: &str) -> Result<FeatureTree> {
        let prompt =
            super::prompts::FEATURE_EXTRACTION_PROMPT.replace("{description}", description);

        let response: FeatureExtractionResponse = self
            .client
            .complete_json_raw("", &prompt)
            .await
            .and_then(|v| self.client.deserialize(v))?;

        Ok(self.response_to_feature_tree(response))
    }

    fn response_to_feature_tree(&self, response: FeatureExtractionResponse) -> FeatureTree {
        let mut tree = FeatureTree::new(&response.root_name);

        for category in response.categories {
            let mut category_node = rpg_encoder::FeatureNode::new(&category.name);

            if let Some(desc) = &category.description {
                category_node = category_node.with_description(desc);
            }

            for feature in &category.features {
                category_node.add_feature(feature);
            }

            for subcategory in &category.subcategories {
                let mut sub_node = rpg_encoder::FeatureNode::new(&subcategory.name);
                for feature in &subcategory.features {
                    sub_node.add_feature(feature);
                }
                category_node.add_child(sub_node);
            }

            tree.root.add_child(category_node);
        }

        tree
    }

    async fn refactor_features(&self, feature_tree: &FeatureTree) -> Result<ComponentPlan> {
        let features: Vec<String> = feature_tree
            .all_features()
            .into_iter()
            .map(String::from)
            .collect();

        let prompt =
            super::prompts::FEATURE_REFACTORING_PROMPT.replace("{features}", &features.join("\n"));

        let response: RefactoringResponse = self
            .client
            .complete_json_raw("", &prompt)
            .await
            .and_then(|v| self.client.deserialize(v))?;

        Ok(self.response_to_component_plan(response))
    }

    fn response_to_component_plan(&self, response: RefactoringResponse) -> ComponentPlan {
        let components: Vec<rpg_encoder::Component> = response
            .components
            .into_iter()
            .map(|c| {
                let mut component = rpg_encoder::Component::new(&c.name, &c.description);
                for feature in &c.features {
                    component.subtree.add_feature(feature);
                }
                component
            })
            .collect();

        ComponentPlan::new(components)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TargetLanguage;

    fn create_test_plan() -> GenerationPlan {
        let mut tree = FeatureTree::new("test");
        tree.root.add_feature("feature1");

        let mut component = rpg_encoder::Component::new("test_component", "Test component");
        component.subtree.add_feature("feature1");

        GenerationPlan::new(
            GenerationRequest::new("Test", TargetLanguage::Rust),
            tree,
            ComponentPlan::new(vec![component]),
        )
    }

    #[test]
    fn test_generation_plan_creation() {
        let plan = create_test_plan();
        assert!(!plan.feature_tree.all_features().is_empty());
    }

    fn create_builder() -> PropertyLevelBuilder {
        let config = crate::llm::LlmConfig::new("test-key");
        let client = OpenAIClient::new(config).unwrap();
        PropertyLevelBuilder::new(client)
    }

    #[test]
    fn test_response_to_feature_tree_with_categories() {
        let builder = create_builder();
        let response = FeatureExtractionResponse {
            root_name: "game engine".to_string(),
            categories: vec![
                CategoryResponse {
                    name: "rendering".to_string(),
                    description: Some("Graphics rendering".to_string()),
                    features: vec!["shading".to_string(), "lighting".to_string()],
                    subcategories: vec![],
                },
                CategoryResponse {
                    name: "physics".to_string(),
                    description: None,
                    features: vec!["collision".to_string()],
                    subcategories: vec![],
                },
            ],
        };

        let tree = builder.response_to_feature_tree(response);
        assert_eq!(tree.root.name, "game engine");
        assert_eq!(tree.root.children.len(), 2);

        let rendering = &tree.root.children[0];
        assert_eq!(rendering.name, "rendering");
        assert_eq!(rendering.description.as_deref(), Some("Graphics rendering"));
        assert_eq!(rendering.features, vec!["shading", "lighting"]);

        let physics = &tree.root.children[1];
        assert_eq!(physics.name, "physics");
        assert!(physics.description.is_none());
        assert_eq!(physics.features, vec!["collision"]);

        let all_features = tree.all_features();
        assert!(all_features.contains(&"shading"));
        assert!(all_features.contains(&"lighting"));
        assert!(all_features.contains(&"collision"));
    }

    #[test]
    fn test_response_to_feature_tree_with_subcategories() {
        let builder = create_builder();
        let response = FeatureExtractionResponse {
            root_name: "app".to_string(),
            categories: vec![CategoryResponse {
                name: "networking".to_string(),
                description: None,
                features: vec!["http_client".to_string()],
                subcategories: vec![
                    SubcategoryResponse {
                        name: "tcp".to_string(),
                        features: vec!["connection_pool".to_string()],
                    },
                    SubcategoryResponse {
                        name: "tls".to_string(),
                        features: vec![],
                    },
                ],
            }],
        };

        let tree = builder.response_to_feature_tree(response);
        let networking = &tree.root.children[0];
        assert_eq!(networking.children.len(), 2);
        assert_eq!(networking.children[0].name, "tcp");
        assert_eq!(networking.children[0].features, vec!["connection_pool"]);
        assert_eq!(networking.children[1].name, "tls");
        assert!(networking.children[1].features.is_empty());

        let all_features = tree.all_features();
        assert!(all_features.contains(&"http_client"));
        assert!(all_features.contains(&"connection_pool"));
    }

    #[test]
    fn test_response_to_feature_tree_empty_categories() {
        let builder = create_builder();
        let response = FeatureExtractionResponse {
            root_name: "empty".to_string(),
            categories: vec![],
        };

        let tree = builder.response_to_feature_tree(response);
        assert_eq!(tree.root.name, "empty");
        assert!(tree.root.children.is_empty());
        assert!(tree.all_features().is_empty());
    }

    #[test]
    fn test_response_to_component_plan_single() {
        let builder = create_builder();
        let response = RefactoringResponse {
            components: vec![ComponentResponse {
                name: "renderer".to_string(),
                description: "Handles rendering".to_string(),
                features: vec!["shading".to_string(), "lighting".to_string()],
            }],
        };

        let plan = builder.response_to_component_plan(response);
        assert_eq!(plan.components.len(), 1);
        assert_eq!(plan.components[0].name, "renderer");
        assert_eq!(plan.components[0].description, "Handles rendering");
        assert_eq!(plan.components[0].all_features().len(), 2);
        assert!(plan.components[0].all_features().contains(&"shading"));
        assert!(plan.components[0].all_features().contains(&"lighting"));
    }

    #[test]
    fn test_response_to_component_plan_multiple() {
        let builder = create_builder();
        let response = RefactoringResponse {
            components: vec![
                ComponentResponse {
                    name: "physics".to_string(),
                    description: "Physics simulation".to_string(),
                    features: vec!["collision".to_string()],
                },
                ComponentResponse {
                    name: "audio".to_string(),
                    description: "Audio playback".to_string(),
                    features: vec!["sound_effects".to_string(), "music".to_string()],
                },
            ],
        };

        let plan = builder.response_to_component_plan(response);
        assert_eq!(plan.components.len(), 2);
        assert_eq!(plan.components[0].name, "physics");
        assert_eq!(plan.components[0].all_features().len(), 1);
        assert_eq!(plan.components[1].name, "audio");
        assert_eq!(plan.components[1].all_features().len(), 2);
    }
}
