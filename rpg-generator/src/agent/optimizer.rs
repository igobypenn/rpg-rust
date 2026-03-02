//! Agent-specific prompt optimizers.

use super::capabilities::AgentCapabilities;
use super::prompt::{PromptFormat, RenderedPrompt};

/// Base trait for prompt optimizers.
pub trait PromptOptimizer: Send + Sync {
    /// Optimize a rendered prompt for agent-specific preferences.
    fn optimize(&self, prompt: &mut RenderedPrompt);
}

/// Optimizer for Claude - adds thinking blocks, markdown format.
#[allow(dead_code)]
pub struct ClaudeOptimizer;

impl ClaudeOptimizer {
    /// Create a new Claude optimizer.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClaudeOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptOptimizer for ClaudeOptimizer {
    fn optimize(&self, prompt: &mut RenderedPrompt) {
        // Claude prefers markdown with thinking blocks
        prompt.format = PromptFormat::Markdown;

        // Add thinking block for extended reasoning
        prompt.content = format!(
            "<thinking>\nAnalyze this task carefully before responding.\n\
             Consider the requirements, constraints, and best practices.\n\
             Plan your implementation approach.\n</thinking>\n\n{}",
            prompt.content
        );

        // Add hints
        prompt
            .metadata
            .agent_hints
            .push("Use artifacts for structured output".to_string());
        prompt
            .metadata
            .agent_hints
            .push("Markdown formatting with clear headers is preferred".to_string());
    }
}

/// Optimizer for Trae-agent - adds structured format, tool hints.
#[allow(dead_code)]
pub struct TraeOptimizer;

impl TraeOptimizer {
    /// Create a new Trae optimizer.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }
}

impl Default for TraeOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptOptimizer for TraeOptimizer {
    fn optimize(&self, prompt: &mut RenderedPrompt) {
        // Trae prefers structured JSON format
        prompt.format = PromptFormat::Structured;

        // Add tool hints
        prompt
            .metadata
            .agent_hints
            .push("Use available tools: bash, edit".to_string());
        prompt
            .metadata
            .agent_hints
            .push("JSON schema for structured output".to_string());
    }
}

/// Optimizer for OpenCode - adds goal-oriented structure.
pub struct OpenCodeOptimizer;

impl OpenCodeOptimizer {
    /// Create a new OpenCode optimizer.
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpenCodeOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptOptimizer for OpenCodeOptimizer {
    fn optimize(&self, prompt: &mut RenderedPrompt) {
        // OpenCode prefers markdown with clear goals
        prompt.format = PromptFormat::Markdown;

        // Add goal-oriented structure
        prompt.content = format!(
            "## Goal\n\
             Complete this task with high quality. Work step by step and be thorough.\n\n\
             ## Task\n{}\n\n\
             ## Guidelines\n\
             - Follow the interface specification exactly\n\
             - Write clean, well-documented code\n\
             - Ensure all tests pass",
            prompt.content
        );

        // Add hints
        prompt
            .metadata
            .agent_hints
            .push("Multi-agent orchestration available".to_string());
        prompt
            .metadata
            .agent_hints
            .push("Break complex tasks into subtasks".to_string());
    }
}

/// Select optimizer based on agent capabilities.
#[allow(dead_code)]
pub fn select_optimizer(caps: &AgentCapabilities) -> Box<dyn PromptOptimizer> {
    if caps.supports_extended_thinking {
        Box::new(ClaudeOptimizer::new())
    } else if caps.supports_docker_isolation {
        Box::new(TraeOptimizer::new())
    } else {
        Box::new(OpenCodeOptimizer::new())
    }
}

#[cfg(test)]
mod tests {
    use super::super::prompt::PromptMetadata;
    use super::*;
    use crate::types::Phase;

    fn create_test_prompt() -> RenderedPrompt {
        RenderedPrompt {
            content: "Test prompt".to_string(),
            format: PromptFormat::Plain,
            metadata: PromptMetadata {
                phase: Phase::CodeGeneration,
                agent_hints: vec![],
            },
        }
    }

    #[test]
    fn test_claude_optimizer_adds_thinking() {
        let mut prompt = create_test_prompt();
        let optimizer = ClaudeOptimizer::new();
        optimizer.optimize(&mut prompt);

        assert!(prompt.content.contains("<thinking>"));
        assert!(matches!(prompt.format, PromptFormat::Markdown));
    }

    #[test]
    fn test_opencode_optimizer_adds_goal() {
        let mut prompt = create_test_prompt();
        let optimizer = OpenCodeOptimizer::new();
        optimizer.optimize(&mut prompt);

        assert!(prompt.content.contains("## Goal"));
        assert!(matches!(prompt.format, PromptFormat::Markdown));
    }

    #[test]
    fn test_trae_optimizer_sets_structured() {
        let mut prompt = create_test_prompt();
        let optimizer = TraeOptimizer::new();
        optimizer.optimize(&mut prompt);

        assert!(matches!(prompt.format, PromptFormat::Structured));
    }
}
