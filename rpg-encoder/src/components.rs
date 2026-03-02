//! Component types for feature grouping.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::features::FeatureNode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub name: String,
    pub description: String,
    pub subtree: FeatureNode,
    pub suggested_directory: PathBuf,
    pub priority: usize,
}

impl Component {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            subtree: FeatureNode::new(name),
            suggested_directory: PathBuf::from(format!("src/{}/", name.replace('.', "/"))),
            priority: 0,
        }
    }

    pub fn with_directory(mut self, dir: PathBuf) -> Self {
        self.suggested_directory = dir;
        self
    }

    pub fn with_priority(mut self, priority: usize) -> Self {
        self.priority = priority;
        self
    }

    pub fn all_features(&self) -> Vec<&str> {
        let mut features = Vec::new();
        self.collect_features(&self.subtree, &mut features);
        features
    }

    fn collect_features<'a>(&'a self, node: &'a FeatureNode, features: &mut Vec<&'a str>) {
        features.extend(node.features.iter().map(|s| s.as_str()));
        for child in &node.children {
            self.collect_features(child, features);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentPlan {
    pub components: Vec<Component>,
    pub feature_coverage: HashMap<String, String>,
}

impl ComponentPlan {
    pub fn new(components: Vec<Component>) -> Self {
        let feature_coverage = Self::build_coverage(&components);
        Self {
            components,
            feature_coverage,
        }
    }

    fn build_coverage(components: &[Component]) -> HashMap<String, String> {
        let mut coverage = HashMap::new();
        for component in components {
            for feature in component.all_features() {
                coverage.insert(feature.to_string(), component.name.clone());
            }
        }
        coverage
    }

    pub fn validate(&self) -> ValidationResult {
        let mut issues = Vec::new();

        let mut feature_counts: HashMap<&str, usize> = HashMap::new();
        for component in &self.components {
            for feature in component.all_features() {
                *feature_counts.entry(feature).or_insert(0) += 1;
            }
        }

        for (feature, count) in &feature_counts {
            if *count > 1 {
                let components: Vec<_> = self
                    .components
                    .iter()
                    .filter(|c| c.all_features().contains(feature))
                    .map(|c| c.name.clone())
                    .collect();

                issues.push(ValidationIssue::DuplicateAssignment {
                    feature: feature.to_string(),
                    components,
                });
            }
        }

        let vague_names = [
            "core", "misc", "util", "utils", "other", "common", "general", "helper", "helpers",
        ];
        for component in &self.components {
            let lower = component.name.to_lowercase();
            for vague in &vague_names {
                if lower
                    .split(['.', '_', '-'])
                    .any(|part| part == *vague)
                {
                    issues.push(ValidationIssue::VagueComponentName(component.name.clone()));
                    break;
                }
            }
        }

        for component in &self.components {
            if component.all_features().is_empty() {
                issues.push(ValidationIssue::EmptyComponent(component.name.clone()));
            }
        }

        if self.components.len() > 12 {
            issues.push(ValidationIssue::TooManyComponents(self.components.len()));
        }

        ValidationResult { issues }
    }

    pub fn component_for(&self, feature: &str) -> Option<&Component> {
        self.feature_coverage
            .get(feature)
            .and_then(|name| self.components.iter().find(|c| &c.name == name))
    }
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub issues: Vec<ValidationIssue>,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.issues.iter().all(|i| !i.is_error())
    }

    pub fn errors(&self) -> Vec<&ValidationIssue> {
        self.issues.iter().filter(|i| i.is_error()).collect()
    }

    pub fn warnings(&self) -> Vec<&ValidationIssue> {
        self.issues.iter().filter(|i| i.is_warning()).collect()
    }
}

#[derive(Debug, Clone)]
pub enum ValidationIssue {
    DuplicateAssignment {
        feature: String,
        components: Vec<String>,
    },
    VagueComponentName(String),
    EmptyComponent(String),
    TooManyComponents(usize),
    TooFewComponents(usize),
}

impl ValidationIssue {
    pub fn is_error(&self) -> bool {
        matches!(self, Self::DuplicateAssignment { .. })
    }

    pub fn is_warning(&self) -> bool {
        !self.is_error()
    }

    pub fn message(&self) -> String {
        match self {
            Self::DuplicateAssignment {
                feature,
                components,
            } => {
                format!(
                    "Feature '{}' assigned to multiple components: {}",
                    feature,
                    components.join(", ")
                )
            }
            Self::VagueComponentName(name) => {
                format!(
                    "Component '{}' has a vague name, consider being more specific",
                    name
                )
            }
            Self::EmptyComponent(name) => format!("Component '{}' has no features", name),
            Self::TooManyComponents(n) => {
                format!("Too many components ({}), consider consolidating", n)
            }
            Self::TooFewComponents(n) => {
                format!("Too few components ({}), may be too coarse-grained", n)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_features() {
        let mut component = Component::new("gameplay.core", "Core gameplay logic");
        component.subtree.add_feature("movement");
        component.subtree.add_feature("collision");

        let features = component.all_features();
        assert_eq!(features.len(), 2);
    }

    #[test]
    fn test_validation_detects_vague_names() {
        let component = Component::new("core", "Core stuff");
        let plan = ComponentPlan::new(vec![component]);

        let result = plan.validate();
        assert!(!result.issues.is_empty());
        assert!(result
            .issues
            .iter()
            .any(|i| matches!(i, ValidationIssue::VagueComponentName(_))));
    }

    #[test]
    fn test_validation_detects_duplicates() {
        let mut c1 = Component::new("gameplay", "Gameplay");
        c1.subtree.add_feature("movement");

        let mut c2 = Component::new("ai", "AI");
        c2.subtree.add_feature("movement");

        let plan = ComponentPlan::new(vec![c1, c2]);
        let result = plan.validate();

        assert!(result
            .issues
            .iter()
            .any(|i| matches!(i, ValidationIssue::DuplicateAssignment { .. })));
    }
}
