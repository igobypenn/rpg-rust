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
        
        let mut plan = GenerationPlan::new(
            request.clone(),
            feature_tree,
            component_plan,
        );
        plan.complete();
        
        self.verifier.verify_generation_plan(&plan)?;
        
        Ok(plan)
    }
    
    async fn extract_features(&self, description: &str) -> Result<FeatureTree> {
        let prompt = super::prompts::FEATURE_EXTRACTION_PROMPT
            .replace("{description}", description);
        
        let response: FeatureExtractionResponse = self.client
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
        let features: Vec<String> = feature_tree.all_features().into_iter().map(String::from).collect();
        
        let prompt = super::prompts::FEATURE_REFACTORING_PROMPT
            .replace("{features}", &features.join("\n"));
        
        let response: RefactoringResponse = self.client
            .complete_json_raw("", &prompt)
            .await
            .and_then(|v| self.client.deserialize(v))?;
        
        Ok(self.response_to_component_plan(response))
    }
    
    fn response_to_component_plan(&self, response: RefactoringResponse) -> ComponentPlan {
        let components: Vec<rpg_encoder::Component> = response.components
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
}
