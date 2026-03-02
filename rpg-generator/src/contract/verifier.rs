//! Contract verifier for phase transitions.

use std::collections::HashSet;

use crate::error::{ContractViolation, GeneratorError, GuidanceRequest, Result, ViolationSeverity};
use crate::{ArchitectureDesign, GenerationPlan};

pub struct ContractVerifier {
    max_auto_fix_retries: usize,
}

impl ContractVerifier {
    pub fn new() -> Self {
        Self {
            max_auto_fix_retries: 2,
        }
    }

    pub fn with_max_retries(mut self, retries: usize) -> Self {
        self.max_auto_fix_retries = retries;
        self
    }

    pub fn verify_generation_plan(&self, plan: &GenerationPlan) -> Result<()> {
        let violations = self.check_generation_plan(plan);
        self.handle_violations(violations)
    }

    pub fn verify_architecture_design(
        &self,
        design: &ArchitectureDesign,
        plan: &GenerationPlan,
    ) -> Result<()> {
        let violations = self.check_architecture_design(design, plan);
        self.handle_violations(violations)
    }

    fn check_generation_plan(&self, plan: &GenerationPlan) -> Vec<ContractViolation> {
        let mut violations = Vec::new();

        if plan.feature_tree.all_features().is_empty() {
            violations.push(ContractViolation::InvalidFeatureTree {
                reason: "Feature tree has no features".to_string(),
            });
        }

        let mut seen_names = HashSet::new();
        for component in &plan.component_plan.components {
            if !seen_names.insert(&component.name) {
                violations.push(ContractViolation::DuplicateName {
                    name: component.name.clone(),
                    context: "Component names must be unique".to_string(),
                });
            }
        }

        let features_in_tree: HashSet<_> = plan.feature_tree.all_features().into_iter().collect();
        for component in &plan.component_plan.components {
            for feature in component.all_features() {
                if !features_in_tree.contains(feature) {
                    violations.push(ContractViolation::MissingFeature {
                        component: component.name.clone(),
                        feature: feature.to_string(),
                    });
                }
            }
        }

        violations
    }

    fn check_architecture_design(
        &self,
        design: &ArchitectureDesign,
        _plan: &GenerationPlan,
    ) -> Vec<ContractViolation> {
        let mut violations = Vec::new();

        let mut file_owners: std::collections::HashMap<std::path::PathBuf, Vec<String>> =
            std::collections::HashMap::new();
        for path in design.interfaces.keys() {
            file_owners.entry(path.clone()).or_default();
        }

        for task in design.task_plan.all_tasks() {
            file_owners
                .entry(task.file_path.clone())
                .or_default()
                .push(task.subtree.clone());
        }

        for (path, owners) in &file_owners {
            if owners.len() > 1 {
                violations.push(ContractViolation::FileConflict {
                    path: path.clone(),
                    claimants: owners.clone(),
                });
            }
        }

        for (type_name, conflicts) in self.find_type_conflicts(&design.type_registry) {
            violations.push(ContractViolation::TypeConflict {
                type_name,
                definitions: conflicts,
            });
        }

        violations
    }

    fn find_type_conflicts(
        &self,
        registry: &std::collections::HashMap<String, crate::types::TypeDefinition>,
    ) -> Vec<(String, Vec<String>)> {
        let mut by_name: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for (name, def) in registry {
            by_name
                .entry(name.clone())
                .or_default()
                .push(def.source_file.display().to_string());
        }

        by_name
            .into_iter()
            .filter(|(_, files)| files.len() > 1)
            .collect()
    }

    fn handle_violations(&self, violations: Vec<ContractViolation>) -> Result<()> {
        if violations.is_empty() {
            return Ok(());
        }

        let (auto_fixable, requires_attention): (Vec<_>, Vec<_>) = violations
            .into_iter()
            .partition(|v| matches!(v.severity(), ViolationSeverity::AutoFixable { .. }));

        if !auto_fixable.is_empty() {
            for violation in &auto_fixable {
                tracing::warn!("Auto-fixable violation: {}", violation);
            }
        }

        if let Some(violation) = requires_attention.into_iter().next() {
            let guidance = match violation.severity() {
                ViolationSeverity::RequiresGuidance => Some(GuidanceRequest {
                    explanation: Self::explain_violation(&violation),
                    suggested_fixes: Self::suggest_fixes(&violation),
                    user_action: crate::error::UserAction::ChooseOption {
                        options: vec![
                            "Retry with modified prompt".to_string(),
                            "Abort".to_string(),
                        ],
                    },
                    violation: violation.clone(),
                }),
                _ => None,
            };

            return Err(GeneratorError::ContractViolation {
                violation,
                guidance,
            });
        }

        Ok(())
    }

    fn explain_violation(violation: &ContractViolation) -> String {
        match violation {
            ContractViolation::CircularDependency { cycle } => {
                format!(
                    "Circular dependency detected: {}. This will cause compilation failures.",
                    cycle.join(" → ")
                )
            }
            ContractViolation::TypeConflict {
                type_name,
                definitions,
            } => {
                format!(
                    "Type '{}' is defined in multiple files: {}. Each type should have a single definition.",
                    type_name,
                    definitions.join(", ")
                )
            }
            ContractViolation::FileConflict { path, claimants } => {
                format!(
                    "File '{}' is claimed by multiple components: {}. Each file should belong to exactly one component.",
                    path.display(),
                    claimants.join(", ")
                )
            }
            _ => violation.to_string(),
        }
    }

    fn suggest_fixes(violation: &ContractViolation) -> Vec<String> {
        match violation {
            ContractViolation::CircularDependency { cycle } => {
                vec![
                    format!(
                        "Extract shared logic from {} to a separate module",
                        cycle.first().unwrap_or(&String::new())
                    ),
                    "Use dependency injection to break the cycle".to_string(),
                    "Reconsider the module boundaries".to_string(),
                ]
            }
            ContractViolation::TypeConflict { type_name, .. } => {
                vec![
                    format!(
                        "Consolidate '{}' definitions into a shared types module",
                        type_name
                    ),
                    format!("Rename one of the '{}' types to avoid conflict", type_name),
                ]
            }
            ContractViolation::FileConflict { path, .. } => {
                vec![
                    format!("Split {} into separate files per component", path.display()),
                    "Assign the file to a single component".to_string(),
                ]
            }
            _ => vec!["Review and adjust the generation request".to_string()],
        }
    }
}

impl Default for ContractVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GenerationRequest, TargetLanguage};
    use rpg_encoder::{Component, ComponentPlan, FeatureTree};

    fn create_test_plan() -> GenerationPlan {
        let mut tree = FeatureTree::new("test");
        tree.root.add_feature("feature1");

        let mut component = Component::new("test_component", "Test component");
        component.subtree.add_feature("feature1");

        GenerationPlan::new(
            GenerationRequest::new("Test", TargetLanguage::Rust),
            tree,
            ComponentPlan::new(vec![component]),
        )
    }

    #[test]
    fn test_verify_valid_plan() {
        let verifier = ContractVerifier::new();
        let plan = create_test_plan();

        assert!(verifier.verify_generation_plan(&plan).is_ok());
    }

    #[test]
    fn test_verify_duplicate_component_names() {
        let verifier = ContractVerifier::new();

        let tree = FeatureTree::new("test");

        let c1 = Component::new("dup", "First");
        let c2 = Component::new("dup", "Second");

        let plan = GenerationPlan::new(
            GenerationRequest::new("Test", TargetLanguage::Rust),
            tree,
            ComponentPlan::new(vec![c1, c2]),
        );

        let result = verifier.verify_generation_plan(&plan);
        assert!(result.is_err());
    }
}
