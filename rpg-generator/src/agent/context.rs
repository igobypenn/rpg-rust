//! Phase-specific contexts for prompt engineering.

use std::collections::HashMap;

use super::capabilities::AgentCapabilities;
use super::prompt::{interpolate, Feedback, PromptFormat, PromptMetadata, RenderedPrompt};
use crate::types::{FileInterface, Phase};
use crate::ImplementationTask;
use rpg_encoder::{Component, FeatureTree};

/// Trait for phase-specific prompt contexts.
pub trait PromptContext: Send + Sync {
    /// Get the phase this context is for.
    fn phase(&self) -> Phase;

    /// Convert context to template variables.
    fn to_variables(&self) -> HashMap<String, String>;
}

/// Phase 1: Property Level - feature extraction context.
#[derive(Debug, Clone)]
pub struct PropertyContext {
    /// Project description.
    pub description: String,
    /// Constraints on the project.
    pub constraints: Vec<String>,
}

impl PromptContext for PropertyContext {
    fn phase(&self) -> Phase {
        Phase::FeaturePlanning
    }

    fn to_variables(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        vars.insert("description".to_string(), self.description.clone());
        vars.insert("constraints".to_string(), self.constraints.join("\n- "));
        vars
    }
}

/// Phase 2: Implementation Level - skeleton and interfaces context.
#[derive(Debug, Clone)]
pub struct ImplementationContext {
    /// Feature tree from Phase 1.
    pub feature_tree: FeatureTree,
    /// Component plan from Phase 1.
    pub components: Vec<Component>,
    /// File interfaces to design.
    pub interfaces: Vec<FileInterface>,
}

impl PromptContext for ImplementationContext {
    fn phase(&self) -> Phase {
        Phase::ArchitectureDesign
    }

    fn to_variables(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        vars.insert(
            "features".to_string(),
            serde_json::to_string(&self.feature_tree).unwrap_or_default(),
        );
        vars.insert(
            "components".to_string(),
            serde_json::to_string(&self.components).unwrap_or_default(),
        );
        vars.insert(
            "interfaces".to_string(),
            self.interfaces
                .iter()
                .map(|i| format!("{}: {} units", i.path.display(), i.units.len()))
                .collect::<Vec<_>>()
                .join("\n"),
        );

        vars
    }
}

/// Phase 3: Code Generation - task with feedback context.
#[derive(Debug, Clone)]
pub struct CodeGenContext {
    /// The implementation task.
    pub task: ImplementationTask,
    /// File interface for this task.
    pub interface: FileInterface,
    /// Feedback from previous iterations.
    pub feedback: Feedback,
}

impl PromptContext for CodeGenContext {
    fn phase(&self) -> Phase {
        Phase::CodeGeneration
    }

    fn to_variables(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        vars.insert(
            "file_path".to_string(),
            self.task.file_path.display().to_string(),
        );
        vars.insert("component".to_string(), self.task.subtree.clone());
        vars.insert(
            "interface".to_string(),
            format!(
                "Units: {}\nImports: {}",
                self.interface
                    .units
                    .iter()
                    .map(|u| u.name.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
                self.interface.imports.join(", ")
            ),
        );

        // Add feedback section if there are issues
        if self.feedback.has_issues() {
            vars.insert("feedback".to_string(), self.feedback.to_prompt_section());
        }

        vars
    }
}

/// Code generation prompt template.
pub const CODE_GEN_TEMPLATE: &str = r#"
Generate code for the following file.

## File
Path: {file_path}
Component: {component}

## Interface
{interface}
{feedback}

## Output Format
Respond with JSON:
```json
{
  "code": "// the implementation code",
  "tests": "// the test code"
}
```
"#;

impl CodeGenContext {
    /// Compile into a rendered prompt for an agent.
    pub fn compile(&self, caps: &AgentCapabilities) -> RenderedPrompt {
        let vars = self.to_variables();
        let content = interpolate(CODE_GEN_TEMPLATE, &vars);

        // Determine format based on agent capabilities
        let format = if caps.supports_extended_thinking {
            PromptFormat::Markdown
        } else if caps.supports_tools {
            PromptFormat::Structured
        } else {
            PromptFormat::Plain
        };

        RenderedPrompt {
            content,
            format,
            metadata: PromptMetadata {
                phase: self.phase(),
                agent_hints: vec![],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_property_context() {
        let ctx = PropertyContext {
            description: "A REST API".to_string(),
            constraints: vec!["Use async".to_string()],
        };
        let vars = ctx.to_variables();
        assert_eq!(vars.get("description"), Some(&"A REST API".to_string()));
    }

    #[test]
    fn test_codegen_context() {
        let task = ImplementationTask::new("task_1", PathBuf::from("src/main.rs"), "core");
        let ctx = CodeGenContext {
            task,
            interface: FileInterface {
                path: PathBuf::from("src/main.rs"),
                units: vec![],
                imports: vec![],
            },
            feedback: Feedback::new(),
        };
        let vars = ctx.to_variables();
        assert!(vars.contains_key("file_path"));
    }
}
