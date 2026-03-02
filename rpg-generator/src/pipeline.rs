//! Pipeline orchestrator for the three-phase generation process.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::checkpoint::CheckpointManager;
use crate::error::Result;
use crate::execution::ExecutionPlan;
use crate::types::{ArchitectureDesign, ExecutionResult, GenerationPlan, GenerationRequest};

#[cfg(not(feature = "opencode"))]
use crate::llm::{LlmConfig, OpenAIClient};

#[cfg(not(feature = "opencode"))]
use crate::phases::{ImplementationLevelBuilder, PropertyLevelBuilder};

#[cfg(feature = "opencode")]
use crate::agent::AgentRegistry;



/// Response from agent for feature extraction.
#[cfg(feature = "opencode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentFeatureResponse {
    root_name: String,
    categories: Vec<AgentCategory>,
}

#[cfg(feature = "opencode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentCategory {
    name: String,
    description: Option<String>,
    features: Vec<String>,
    subcategories: Vec<AgentSubcategory>,
}

#[cfg(feature = "opencode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentSubcategory {
    name: String,
    features: Vec<String>,
}

/// Response from agent for component refactoring.
/// TODO: Used when Phase 1 returns structured component breakdown
#[cfg(feature = "opencode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
struct AgentComponentResponse {
    components: Vec<AgentComponent>,
}

#[cfg(feature = "opencode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
struct AgentComponent {
    name: String,
    description: String,
    features: Vec<String>,
}



/// Response from agent for skeleton design.
#[cfg(feature = "opencode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentSkeletonResponse {
    directories: Vec<String>,
    files: Vec<AgentFileDesign>,
    entry_point: Option<String>,
}

#[cfg(feature = "opencode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentFileDesign {
    path: String,
    purpose: String,
    component: String,
    units: Option<Vec<AgentUnitDesign>>,
}

#[cfg(feature = "opencode")]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentUnitDesign {
    name: String,
    kind: String,
    signature: Option<String>,
    docstring: Option<String>,
    features: Vec<String>,
}

/// RPG Generator - orchestrates the three-phase generation pipeline.
///
/// # Features
///
/// - With `opencode` feature: Uses OpenCode agent for all phases (no LLM API key needed)
/// - Without `opencode` feature: Uses OpenAI API for phases 1-2, requires API key
pub struct RpgGenerator {
    #[cfg(not(feature = "opencode"))]
    config: LlmConfig,
    checkpoint: Option<Arc<RwLock<CheckpointManager>>>,
    output_dir: PathBuf,
    max_test_iterations: usize,
    max_verification_retries: usize,
    verification_threshold: f32,
}

impl RpgGenerator {
    /// Create a new generator.
    ///
    /// With `opencode` feature: No config needed, agent handles all phases.
    /// Without `opencode` feature: Requires LlmConfig with API key.
    #[cfg(feature = "opencode")]
    pub fn new() -> Self {
        Self {
            checkpoint: None,
            output_dir: PathBuf::from("."),
            max_test_iterations: 5,
            max_verification_retries: 3,
            verification_threshold: 0.8,
        }
    }

    #[cfg(not(feature = "opencode"))]
    pub fn new(config: LlmConfig) -> Self {
        Self {
            config,
            checkpoint: None,
            output_dir: PathBuf::from("."),
            max_test_iterations: 5,
            max_verification_retries: 3,
            verification_threshold: 0.8,
        }
    }

    pub fn with_output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.output_dir = dir.into();
        self
    }

    pub fn with_checkpoint(mut self, checkpoint: Arc<RwLock<CheckpointManager>>) -> Self {
        self.checkpoint = Some(checkpoint);
        self
    }

    pub fn with_max_test_iterations(mut self, max: usize) -> Self {
        self.max_test_iterations = max;
        self
    }

    /// Set the maximum number of verification retry iterations.
    pub fn with_max_verification_retries(mut self, max: usize) -> Self {
        self.max_verification_retries = max;
        self
    }

    /// Set the verification similarity threshold (0.0-1.0).
    pub fn with_verification_threshold(mut self, threshold: f32) -> Self {
        self.verification_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub async fn generate(&self, request: GenerationRequest) -> Result<GenerationOutput> {
        tracing::info!("Starting generation for: {}", request.description);

        let phase1_result = self.run_phase1(&request).await?;
        
        if let Some(ref checkpoint) = self.checkpoint {
            let mut mgr = checkpoint.write().await;
            let _ = mgr.set_generation_plan(phase1_result.clone());
            let _ = mgr.advance_phase(crate::types::Phase::ArchitectureDesign);
        }

        let phase2_result = self.run_phase2(&phase1_result).await?;
        
        if let Some(ref checkpoint) = self.checkpoint {
            let mut mgr = checkpoint.write().await;
            let _ = mgr.set_architecture_design(phase2_result.clone());
            let _ = mgr.advance_phase(crate::types::Phase::CodeGeneration);
        }

        let mut phase3_result = self.run_phase3(&phase2_result).await?;
        
        // === Verification: Close the loop with retry ===
        // Verify generated code against planned RPG with up to max_verification_retries attempts
        let mut verification_attempts = 0;
        let mut final_verification_passed = false;
        
        if let Some(ref planned_rpg) = phase1_result.planned_rpg {
            let output_dir = &self.output_dir;
            
            for retry in 0..self.max_verification_retries {
                verification_attempts = retry + 1;
                tracing::info!(
                    "Phase 4: Verification - Closing the loop (attempt {}/{})",
                    verification_attempts,
                    self.max_verification_retries
                );
                
                match crate::verification::GraphVerifier::new() {
                    Ok(mut verifier) => {
                        match verifier.verify(output_dir, planned_rpg) {
                            Ok(verification_result) => {
                                let similarity_percent = verification_result.similarity * 100.0;
                                tracing::info!(
                                    "Verification complete: similarity={:.2}%, passed={}, threshold={:.0}%",
                                    similarity_percent,
                                    verification_result.passed,
                                    self.verification_threshold * 100.0
                                );
                                
                                // Check if verification passed our threshold
                                if verification_result.similarity >= self.verification_threshold {
                                    final_verification_passed = true;
                                    
                                    // Get the generated graph from the verifier
                                    if let Ok(encode_result) = rpg_encoder::RpgEncoder::new()
                                        .and_then(|mut e| e.encode(output_dir))
                                    {
                                        phase3_result.final_graph = Some(encode_result.graph);
                                    }
                                    
                                    tracing::info!("Verification passed after {} attempt(s)", verification_attempts);
                                    break;
                                } else {
                                    tracing::warn!(
                                        "Verification failed: similarity {:.2}% below threshold {:.0}%",
                                        similarity_percent,
                                        self.verification_threshold * 100.0
                                    );
                                    
                                    // If not the last attempt, log what's missing for the next iteration
                                    if retry + 1 < self.max_verification_retries {
                                        if !verification_result.missing_features.is_empty() {
                                            tracing::info!(
                                                "Missing features to address: {:?}",
                                                verification_result.missing_features.iter().take(5).collect::<Vec<_>>()
                                            );
                                        }
                                        
                                        // Re-run phase 3 with the missing features as additional context
                                        tracing::info!("Re-running Phase 3 code generation to address gaps...");
                                        
                                        // Note: In a full implementation, we would pass missing_features
                                        // back to the agent as context. For now, we just retry.
                                        match self.run_phase3(&phase2_result).await {
                                            Ok(new_result) => {
                                                phase3_result = new_result;
                                            }
                                            Err(e) => {
                                                tracing::error!("Phase 3 retry failed: {}", e);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Verification error: {}", e);
                                
                                if retry + 1 < self.max_verification_retries {
                                    // Retry phase 3 on verification error
                                    match self.run_phase3(&phase2_result).await {
                                        Ok(new_result) => {
                                            phase3_result = new_result;
                                        }
                                        Err(e) => {
                                            tracing::error!("Phase 3 retry failed: {}", e);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create verifier: {}", e);
                        break;
                    }
                }
            }
            
            if !final_verification_passed && verification_attempts > 0 {
                tracing::warn!(
                    "Verification did not pass after {} attempts. Proceeding with best-effort result.",
                    verification_attempts
                );
                
                // Still try to encode the final result
                if let Ok(encode_result) = rpg_encoder::RpgEncoder::new()
                    .and_then(|mut e| e.encode(output_dir))
                {
                    phase3_result.final_graph = Some(encode_result.graph);
                }
            }
        }
        
        if let Some(ref checkpoint) = self.checkpoint {
            let mut mgr = checkpoint.write().await;
            let _ = mgr.set_execution_result(phase3_result.clone());
            let _ = mgr.advance_phase(crate::types::Phase::Completed);
        }
        Ok(GenerationOutput {
            request,
            plan: phase1_result,
            design: phase2_result,
            result: phase3_result,
        })
    }

    // === Phase 1: Property Level ===

    #[cfg(feature = "opencode")]
    async fn run_phase1(&self, request: &GenerationRequest) -> Result<GenerationPlan> {
        tracing::info!("Phase 1: Property Level - Feature extraction (via agent)");
        
        let mut registry = AgentRegistry::new();
        let agent = registry.take_default();
        
        if !agent.is_available() {
            return Err(crate::GeneratorError::AgentNotAvailable(
                format!("Agent '{}' CLI not found in PATH. Install it first.", agent.name())
            ));
        }

        let prompt = crate::agent::RenderedPrompt {
            content: format!(
                "Extract features and components from this project description:\n\n{}\n\n\
                 Respond with JSON containing:\n\
                 - root_name: project name\n\
                 - categories: array of {{name, description, features[], subcategories[]}}\n\
                 - components: array of {{name, description, features[]}}",
                request.description
            ),
            format: crate::agent::PromptFormat::Json,
            metadata: crate::agent::PromptMetadata {
                phase: crate::types::Phase::FeaturePlanning,
                agent_hints: vec![],
            },
        };

        let output = agent.execute(&prompt).await?;
        
        // Parse the agent response
        let json_str = output.as_json()
            .map(|v| v.to_string())
            .unwrap_or_else(|| output.to_text());
        
        let (feature_tree, component_plan) = self.parse_phase1_response(&json_str, &request.description)?;
        
        let mut plan = GenerationPlan::new(
            request.clone(),
            feature_tree,
            component_plan,
        );
        
        // Create planned RPG from features for verification
        let planned_rpg = crate::centroid_expander::create_planned_graph_from_features(&plan.feature_tree);
        plan.set_planned_rpg(planned_rpg);
        
        Ok(plan)
    }
    /// Parse Phase 1 agent response into FeatureTree and ComponentPlan.
    #[cfg(feature = "opencode")]
    fn parse_phase1_response(
        &self,
        json_str: &str,
        description: &str,
    ) -> Result<(rpg_encoder::FeatureTree, rpg_encoder::ComponentPlan)> {
        // Try to parse as combined response first
        if let Ok(combined) = serde_json::from_str::<serde_json::Value>(json_str) {
            let feature_tree = if let Some(categories) = combined.get("categories") {
                self.json_to_feature_tree(categories, description)
            } else if let Ok(response) = serde_json::from_str::<AgentFeatureResponse>(json_str) {
                self.response_to_feature_tree(response)
            } else {
                rpg_encoder::FeatureTree::new("project")
            };
            
            let component_plan = if let Some(components) = combined.get("components") {
                self.json_to_component_plan(components)
            } else {
                self.infer_components_from_features(&feature_tree)
            };
            
            return Ok((feature_tree, component_plan));
        }
        
        // Fallback: try direct parsing
        if let Ok(response) = serde_json::from_str::<AgentFeatureResponse>(json_str) {
            let feature_tree = self.response_to_feature_tree(response);
            let component_plan = self.infer_components_from_features(&feature_tree);
            return Ok((feature_tree, component_plan));
        }
        
        // Ultimate fallback: create basic plan from description
        tracing::warn!("Failed to parse agent response, creating basic plan");
        let feature_tree = rpg_encoder::FeatureTree::new("project");
        let component_plan = rpg_encoder::ComponentPlan::new(vec![]);
        Ok((feature_tree, component_plan))
    }
    
    /// Convert AgentFeatureResponse to FeatureTree.
    #[cfg(feature = "opencode")]
    fn response_to_feature_tree(&self, response: AgentFeatureResponse) -> rpg_encoder::FeatureTree {
        let mut tree = rpg_encoder::FeatureTree::new(&response.root_name);
        
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
    
    /// Convert JSON value to FeatureTree.
    #[cfg(feature = "opencode")]
    fn json_to_feature_tree(&self, value: &serde_json::Value, description: &str) -> rpg_encoder::FeatureTree {
        let mut tree = rpg_encoder::FeatureTree::new("project");
        
        if let Some(categories) = value.as_array() {
            for cat in categories {
                if let Some(name) = cat.get("name").and_then(|n| n.as_str()) {
                    let mut category_node = rpg_encoder::FeatureNode::new(name);
                    
                    if let Some(desc) = cat.get("description").and_then(|d| d.as_str()) {
                        category_node = category_node.with_description(desc);
                    }
                    
                    if let Some(features) = cat.get("features").and_then(|f| f.as_array()) {
                        for feature in features {
                            if let Some(feat_name) = feature.as_str() {
                                category_node.add_feature(feat_name);
                            }
                        }
                    }
                    
                    tree.root.add_child(category_node);
                }
            }
        }
        
        // If no categories found, create a basic feature from description
        if tree.root.children.is_empty() {
            let mut feature_node = rpg_encoder::FeatureNode::new("main");
            feature_node.add_feature(description);
            tree.root.add_child(feature_node);
        }
        
        tree
    }
    
    /// Convert JSON value to ComponentPlan.
    #[cfg(feature = "opencode")]
    fn json_to_component_plan(&self, value: &serde_json::Value) -> rpg_encoder::ComponentPlan {
        let mut components = Vec::new();
        
        if let Some(comps) = value.as_array() {
            for comp in comps {
                if let Some(name) = comp.get("name").and_then(|n| n.as_str()) {
                    let description = comp.get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or_default()
                        .to_string();
                    
                    let mut component = rpg_encoder::Component::new(name, &description);
                    
                    if let Some(features) = comp.get("features").and_then(|f| f.as_array()) {
                        for feature in features {
                            if let Some(feat_name) = feature.as_str() {
                                component.subtree.add_feature(feat_name);
                            }
                        }
                    }
                    
                    components.push(component);
                }
            }
        }
        
        rpg_encoder::ComponentPlan::new(components)
    }
    
    /// Infer components from feature tree when not explicitly provided.
    #[cfg(feature = "opencode")]
    fn infer_components_from_features(&self, feature_tree: &rpg_encoder::FeatureTree) -> rpg_encoder::ComponentPlan {
        let mut components = Vec::new();
        
        for child in &feature_tree.root.children {
            let mut component = rpg_encoder::Component::new(&child.name, &child.description.clone().unwrap_or_default());
            
            for feature in &child.features {
                component.subtree.add_feature(feature);
            }
            
            for subchild in &child.children {
                for feature in &subchild.features {
                    component.subtree.add_feature(feature);
                }
            }
            
            components.push(component);
        }
        
        if components.is_empty() {
            // Create a single "core" component from all features
            let mut core = rpg_encoder::Component::new("core", "Core functionality");
            for feature in feature_tree.all_features() {
                core.subtree.add_feature(feature);
            }
            components.push(core);
        }
        
        rpg_encoder::ComponentPlan::new(components)
    }

    #[cfg(not(feature = "opencode"))]
    async fn run_phase1(&self, request: &GenerationRequest) -> Result<GenerationPlan> {
        tracing::info!("Phase 1: Property Level - Feature extraction");
        
        let client = OpenAIClient::new(self.config.clone())?;
        let builder = PropertyLevelBuilder::new(client);
        builder.build(request).await
    }

    // === Phase 2: Implementation Level ===

    #[cfg(feature = "opencode")]
    async fn run_phase2(&self, plan: &GenerationPlan) -> Result<ArchitectureDesign> {
        tracing::info!("Phase 2: Implementation Level - Architecture design (via agent)");
        
        let mut registry = AgentRegistry::new();
        let agent = registry.take_default();

        // Build component info for prompt
        let component_info: Vec<String> = plan.component_plan.components.iter()
            .map(|c| format!("- {}: {}", c.name, c.description))
            .collect();
        
        let prompt = crate::agent::RenderedPrompt {
            content: format!(
                "Design the architecture and file structure for a project with these components:\n{}\n\n\
                 Respond with JSON containing:\n\
                 - directories: array of directory paths\n\
                 - files: array of {{path, purpose, component, units: [{{name, kind, signature, docstring, features}}]}}\n\
                 - entry_point: main entry file path",
                component_info.join("\n")
            ),
            format: crate::agent::PromptFormat::Json,
            metadata: crate::agent::PromptMetadata {
                phase: crate::types::Phase::ArchitectureDesign,
                agent_hints: vec![],
            },
        };

        let output = agent.execute(&prompt).await?;
        
        // Parse the architecture response
        let design = self.parse_phase2_response(&output, plan)?;
        
        Ok(design)
    }
    
    /// Parse Phase 2 agent response into ArchitectureDesign.
    #[cfg(feature = "opencode")]
    fn parse_phase2_response(
        &self,
        output: &crate::agent::AgentOutput,
        plan: &GenerationPlan,
    ) -> Result<ArchitectureDesign> {
        let json_str = output.as_json()
            .map(|v| v.to_string())
            .unwrap_or_else(|| output.to_text());
        
        // Try to parse skeleton response
        if let Ok(response) = serde_json::from_str::<AgentSkeletonResponse>(&json_str) {
            return Ok(self.skeleton_response_to_design(&response, plan));
        }
        
        // Try to parse as generic JSON
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json_str) {
            return Ok(self.json_to_design(&value, plan));
        }
        
        // Fallback: create basic design from component plan
        tracing::warn!("Failed to parse architecture response, creating basic design");
        Ok(self.create_default_design(plan))
    }
    
    /// Convert AgentSkeletonResponse to ArchitectureDesign.
    #[cfg(feature = "opencode")]
    fn skeleton_response_to_design(
        &self,
        response: &AgentSkeletonResponse,
        plan: &GenerationPlan,
    ) -> ArchitectureDesign {
        let mut skeleton = rpg_encoder::RepoSkeleton::new(PathBuf::from("src"), "rust");
        let mut task_plan = rpg_encoder::TaskPlan::new();
        
        for file_design in &response.files {
            let mut file = rpg_encoder::SkeletonFile::new(
                PathBuf::from(&file_design.path),
                "rust",
            );
            
            if let Some(units) = &file_design.units {
                for unit in units {
                    let kind = match unit.kind.as_str() {
                        "function" => rpg_encoder::UnitKind::Function,
                        "struct" => rpg_encoder::UnitKind::Struct,
                        "enum" => rpg_encoder::UnitKind::Enum,
                        "trait" => rpg_encoder::UnitKind::Trait,
                        "module" => rpg_encoder::UnitKind::Module,
                        "class" => rpg_encoder::UnitKind::Class,
                        "interface" => rpg_encoder::UnitKind::Interface,
                        _ => rpg_encoder::UnitKind::Function,
                    };
                    
                    let mut unit_skeleton = rpg_encoder::UnitSkeleton::new(&unit.name, kind)
                        .with_features(unit.features.clone());
                    
                    if let Some(sig) = &unit.signature {
                        unit_skeleton = unit_skeleton.with_signature(sig);
                    }
                    if let Some(doc) = &unit.docstring {
                        unit_skeleton = unit_skeleton.with_docstring(doc);
                    }
                    
                    file.add_unit(unit_skeleton);
                }
            }
            
            skeleton.add_file(file);
        }
        
        // Create tasks from components
        for component in &plan.component_plan.components {
            let task = rpg_encoder::ImplementationTask::new(
                &format!("task_{}", component.name.replace('.', "_")),
                PathBuf::from(format!("src/{}/mod.rs", component.name.replace('.', "/"))),
                &component.name,
            );
            task_plan.add_batch(&component.name, vec![task]);
        }
        
        ArchitectureDesign::new(uuid::Uuid::new_v4(), skeleton, task_plan)
    }
    
    /// Convert JSON value to ArchitectureDesign.
    #[cfg(feature = "opencode")]
    fn json_to_design(
        &self,
        value: &serde_json::Value,
        plan: &GenerationPlan,
    ) -> ArchitectureDesign {
        let mut skeleton = rpg_encoder::RepoSkeleton::new(PathBuf::from("src"), "rust");
        let mut task_plan = rpg_encoder::TaskPlan::new();
        
        // Parse files array if present
        if let Some(files) = value.get("files").and_then(|f| f.as_array()) {
            for file_val in files {
                if let Some(path) = file_val.get("path").and_then(|p| p.as_str()) {
                    let file = rpg_encoder::SkeletonFile::new(PathBuf::from(path), "rust");
                    skeleton.add_file(file);
                }
            }
        }
        
        // Create tasks from components
        for component in &plan.component_plan.components {
            let task = rpg_encoder::ImplementationTask::new(
                &format!("task_{}", component.name.replace('.', "_")),
                PathBuf::from(format!("src/{}/mod.rs", component.name.replace('.', "/"))),
                &component.name,
            );
            task_plan.add_batch(&component.name, vec![task]);
        }
        
        ArchitectureDesign::new(uuid::Uuid::new_v4(), skeleton, task_plan)
    }
    
    /// Create default design from component plan.
    #[cfg(feature = "opencode")]
    fn create_default_design(&self, plan: &GenerationPlan) -> ArchitectureDesign {
        let mut skeleton = rpg_encoder::RepoSkeleton::new(PathBuf::from("src"), "rust");
        let mut task_plan = rpg_encoder::TaskPlan::new();
        
        for component in &plan.component_plan.components {
            // Create skeleton file for each component
            let file = rpg_encoder::SkeletonFile::new(
                PathBuf::from(format!("src/{}/mod.rs", component.name.replace('.', "/"))),
                "rust",
            );
            skeleton.add_file(file);
            
            // Create task for each component
            let task = rpg_encoder::ImplementationTask::new(
                &format!("task_{}", component.name.replace('.', "_")),
                PathBuf::from(format!("src/{}/mod.rs", component.name.replace('.', "/"))),
                &component.name,
            );
            task_plan.add_batch(&component.name, vec![task]);
        }
        
        ArchitectureDesign::new(uuid::Uuid::new_v4(), skeleton, task_plan)
    }

    #[cfg(not(feature = "opencode"))]
    async fn run_phase2(&self, plan: &GenerationPlan) -> Result<ArchitectureDesign> {
        tracing::info!("Phase 2: Implementation Level - Architecture design");
        
        let client = OpenAIClient::new(self.config.clone())?;
        let builder = ImplementationLevelBuilder::new(client);
        builder.build(plan).await
    }

    // === Phase 3: Code Generation ===

    #[cfg(feature = "opencode")]
    async fn run_phase3(&self, design: &ArchitectureDesign) -> Result<ExecutionResult> {
        tracing::info!("Phase 3: Code Generation - TDD loop (via agent)");
        
        let plan = ExecutionPlan::new(design.clone())
            .with_max_iterations(self.max_test_iterations);
        
        plan.execute().await
    }

    #[cfg(not(feature = "opencode"))]
    async fn run_phase3(&self, design: &ArchitectureDesign) -> Result<ExecutionResult> {
        tracing::info!("Phase 3: Code Generation - TDD loop");
        
        let client = OpenAIClient::new(self.config.clone())?;
        let plan = ExecutionPlan::new_with_client(design.clone(), client)
            .with_max_iterations(self.max_test_iterations);
        
        plan.execute().await
    }
}

impl Default for RpgGenerator {
    #[cfg(feature = "opencode")]
    fn default() -> Self {
        Self::new()
    }

    #[cfg(not(feature = "opencode"))]
    fn default() -> Self {
        Self::new(LlmConfig::default())
    }
}

pub struct GenerationOutput {
    pub request: GenerationRequest,
    pub plan: GenerationPlan,
    pub design: ArchitectureDesign,
    pub result: ExecutionResult,
}

impl GenerationOutput {
    pub fn total_files(&self) -> usize {
        self.result.file_count()
    }

    pub fn completed_tasks(&self) -> usize {
        self.result.completed_count()
    }

    pub fn failed_tasks(&self) -> usize {
        self.result.failed_count()
    }

    pub fn success_rate(&self) -> f32 {
        let total = self.result.task_outcomes.len();
        if total == 0 {
            return 0.0;
        }
        self.result.completed_count() as f32 / total as f32
    }
    
    /// Get the final RPG graph (if verification was run).
    pub fn final_graph(&self) -> Option<&rpg_encoder::RpgGraph> {
        self.result.final_graph.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "opencode")]
    #[test]
    fn test_generator_creation() {
        let generator = RpgGenerator::new();
        assert_eq!(generator.max_test_iterations, 5);
    }

    #[cfg(not(feature = "opencode"))]
    #[test]
    fn test_generator_creation() {
        let config = LlmConfig::new("test-key");
        let generator = RpgGenerator::new(config);
        assert_eq!(generator.max_test_iterations, 5);
    }

    #[cfg(feature = "opencode")]
    #[test]
    fn test_generator_with_options() {
        let generator = RpgGenerator::new()
            .with_max_test_iterations(10)
            .with_output_dir("/tmp/output");
        
        assert_eq!(generator.max_test_iterations, 10);
        assert_eq!(generator.output_dir, PathBuf::from("/tmp/output"));
    }

    #[cfg(not(feature = "opencode"))]
    #[test]
    fn test_generator_with_options() {
        let config = LlmConfig::new("test-key");
        let generator = RpgGenerator::new(config)
            .with_max_test_iterations(10)
            .with_output_dir("/tmp/output");
        
        assert_eq!(generator.max_test_iterations, 10);
        assert_eq!(generator.output_dir, PathBuf::from("/tmp/output"));
    }
    
    #[cfg(feature = "opencode")]
    #[test]
    fn test_parse_feature_response() {
        let generator = RpgGenerator::new();
        let json = r#"{
            "root_name": "test_project",
            "categories": [
                {
                    "name": "auth",
                    "description": "Authentication",
                    "features": ["login", "logout"],
                    "subcategories": []
                }
            ]
        }"#;
        
        let response: AgentFeatureResponse = serde_json::from_str(json).expect("Failed to parse");
        let tree = generator.response_to_feature_tree(response);
        
        assert_eq!(tree.root.name, "test_project");
        assert_eq!(tree.root.children.len(), 1);
        assert_eq!(tree.root.children[0].name, "auth");
    }
    
    #[cfg(feature = "opencode")]
    #[test]
    fn test_infer_components() {
        let generator = RpgGenerator::new();
        let mut tree = rpg_encoder::FeatureTree::new("project");
        let mut auth = rpg_encoder::FeatureNode::new("auth");
        auth.add_feature("login");
        tree.root.add_child(auth);
        
        let plan = generator.infer_components_from_features(&tree);
        assert_eq!(plan.components.len(), 1);
        assert_eq!(plan.components[0].name, "auth");
    }
}
