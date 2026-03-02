//! Prompt system with type-safe contexts and feedback integration.

use serde::Deserialize;
use std::collections::HashMap;

use super::capabilities::AgentCapabilities;
use crate::error::Result;

/// Rendered prompt ready for agent execution.
#[derive(Debug, Clone)]
pub struct RenderedPrompt {
    /// The prompt content.
    pub content: String,
    /// Format hint for the agent.
    pub format: PromptFormat,
    /// Additional metadata.
    pub metadata: PromptMetadata,
}

/// Prompt format hints.
#[derive(Debug, Clone, Copy, Default)]
pub enum PromptFormat {
    /// Markdown format (Claude prefers).
    Markdown,
    /// Structured JSON format (Trae prefers).
    Structured,
    /// Plain text format.
    #[default]
    Plain,
    /// JSON response expected.
    Json,
}

/// Additional prompt metadata.
#[derive(Debug, Clone, Default)]
pub struct PromptMetadata {
    /// Phase this prompt is for.
    pub phase: crate::types::Phase,
    /// Hints for the agent.
    pub agent_hints: Vec<String>,
}

/// Feedback accumulator for iteration.
#[derive(Debug, Clone, Default)]
pub struct Feedback {
    /// Test failures from previous iteration.
    pub test_failures: Vec<TestFailure>,
    /// Type errors from type checking.
    pub type_errors: Vec<TypeError>,
    /// Lint warnings from linter.
    pub lint_warnings: Vec<LintIssue>,
    /// Previous code attempt (for patch mode).
    pub previous_code: Option<String>,
    /// Current iteration number.
    pub iteration: u32,
}

impl Feedback {
    /// Create new empty feedback.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there are any issues.
    pub fn has_issues(&self) -> bool {
        !self.test_failures.is_empty()
            || !self.type_errors.is_empty()
            || !self.lint_warnings.is_empty()
    }

    /// Format feedback as a prompt section.
    pub fn to_prompt_section(&self) -> String {
        let mut section = String::new();

        if !self.test_failures.is_empty() {
            section.push_str(&format!(
                "\n## Test Failures (Iteration {})\n",
                self.iteration
            ));
            for f in &self.test_failures {
                section.push_str(&format!("- **{}**: {}\n", f.test_name, f.message));
            }
        }

        if !self.type_errors.is_empty() {
            section.push_str("\n## Type Errors\n");
            for e in &self.type_errors {
                section.push_str(&format!("- {}:{}: {}\n", e.file, e.line, e.message));
            }
        }

        if !self.lint_warnings.is_empty() {
            section.push_str("\n## Lint Warnings\n");
            for w in &self.lint_warnings {
                section.push_str(&format!("- {}: {}\n", w.code, w.message));
            }
        }

        if let Some(ref code) = self.previous_code {
            section.push_str("\n## Previous Code\n```\n");
            section.push_str(code);
            section.push_str("\n```\n");
        }

        section
    }
}

/// A test failure.
#[derive(Debug, Clone)]
pub struct TestFailure {
    /// Test name.
    pub test_name: String,
    /// Error message.
    pub message: String,
    /// Line number (if available).
    pub line: Option<usize>,
}

/// A type error.
#[derive(Debug, Clone)]
pub struct TypeError {
    /// File path.
    pub file: String,
    /// Line number.
    pub line: usize,
    /// Error message.
    pub message: String,
}

/// A lint issue.
#[derive(Debug, Clone)]
pub struct LintIssue {
    /// Lint code.
    pub code: String,
    /// Message.
    pub message: String,
    /// File path.
    pub file: String,
}

/// Base trait for type-safe prompts.
pub trait Prompt: Send + Sync {
    /// Context type for this prompt.
    type Context;
    /// Output type expected from this prompt.
    type Output: for<'de> Deserialize<'de>;

    /// Get the template string.
    fn template(&self) -> &'static str;

    /// Compile the prompt with context for a specific agent.
    fn compile(&self, ctx: &Self::Context, caps: &AgentCapabilities) -> RenderedPrompt;

    /// Parse the response into the output type.
    fn parse(&self, raw: &str) -> Result<Self::Output>;
}

/// Helper to interpolate template variables.
pub fn interpolate(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{}}}", key), value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_empty() {
        let feedback = Feedback::new();
        assert!(!feedback.has_issues());
    }

    #[test]
    fn test_feedback_with_failures() {
        let feedback = Feedback {
            test_failures: vec![TestFailure {
                test_name: "test_foo".to_string(),
                message: "assertion failed".to_string(),
                line: Some(42),
            }],
            ..Default::default()
        };
        assert!(feedback.has_issues());
    }

    #[test]
    fn test_feedback_to_prompt_section() {
        let feedback = Feedback {
            iteration: 1,
            test_failures: vec![TestFailure {
                test_name: "test_bar".to_string(),
                message: "expected 1, got 2".to_string(),
                line: None,
            }],
            ..Default::default()
        };
        let section = feedback.to_prompt_section();
        assert!(section.contains("Test Failures"));
        assert!(section.contains("test_bar"));
    }
}
