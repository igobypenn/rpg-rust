//! Agent capabilities and output types.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Capabilities that an agent supports.
///
/// Used by the prompt system to optimize prompts for each agent's strengths.
#[derive(Clone, Debug, Default)]
pub struct AgentCapabilities {
    /// Agent supports extended thinking (Claude).
    pub supports_extended_thinking: bool,

    /// Agent runs in Docker isolation (Trae).
    pub supports_docker_isolation: bool,

    /// Agent can orchestrate multiple sub-agents (OpenCode).
    pub supports_multi_agent: bool,

    /// Agent can output structured artifacts (Claude).
    pub supports_artifacts: bool,

    /// Agent has access to tools (bash, edit, etc.) (Trae).
    pub supports_tools: bool,

    /// Maximum context window in tokens (if known).
    pub max_context_tokens: Option<usize>,
}

impl AgentCapabilities {
    /// Create capabilities for OpenCode agent.
    pub fn opencode() -> Self {
        Self {
            supports_multi_agent: true,
            supports_extended_thinking: false,
            supports_docker_isolation: false,
            supports_artifacts: false,
            supports_tools: false,
            max_context_tokens: Some(128_000),
        }
    }

    /// Create capabilities for Trae agent.
    pub fn trae() -> Self {
        Self {
            supports_multi_agent: false,
            supports_extended_thinking: false,
            supports_docker_isolation: true,
            supports_artifacts: false,
            supports_tools: true,
            max_context_tokens: Some(200_000),
        }
    }

    /// Create capabilities for Claude agent.
    pub fn claude() -> Self {
        Self {
            supports_multi_agent: false,
            supports_extended_thinking: true,
            supports_docker_isolation: false,
            supports_artifacts: true,
            supports_tools: false,
            max_context_tokens: Some(200_000),
        }
    }
}

/// Output from agent execution.
#[derive(Debug, Clone)]
pub enum AgentOutput {
    /// Plain text response.
    Text(String),

    /// JSON response (parsed).
    Json(serde_json::Value),

    /// File patches (from trae-agent).
    Patches(Vec<Patch>),
}

impl AgentOutput {
    /// Get output as text.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            Self::Json(_v) => None,
            Self::Patches(_) => None,
        }
    }

    /// Get output as JSON.
    pub fn as_json(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Text(_) => None,
            Self::Json(v) => Some(v),
            Self::Patches(_) => None,
        }
    }

    /// Convert to text, serializing JSON if necessary.
    pub fn to_text(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Json(v) => v.to_string(),
            Self::Patches(patches) => patches
                .iter()
                .map(|p| format!("=== {} ===\n{}", p.path.display(), p.content))
                .collect::<Vec<_>>()
                .join("\n\n"),
        }
    }
}

/// A file patch from an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    /// Target file path.
    pub path: PathBuf,

    /// Patch content (unified diff or full content).
    pub content: String,

    /// Patch operation type.
    #[serde(default)]
    pub operation: PatchOperation,
}

/// Type of patch operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum PatchOperation {
    /// Create new file.
    #[default]
    Create,
    /// Modify existing file.
    Modify,
    /// Delete file.
    Delete,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opencode_capabilities() {
        let caps = AgentCapabilities::opencode();
        assert!(caps.supports_multi_agent);
        assert!(!caps.supports_extended_thinking);
    }

    #[test]
    fn test_claude_capabilities() {
        let caps = AgentCapabilities::claude();
        assert!(caps.supports_extended_thinking);
        assert!(caps.supports_artifacts);
    }

    #[test]
    fn test_trae_capabilities() {
        let caps = AgentCapabilities::trae();
        assert!(caps.supports_docker_isolation);
        assert!(caps.supports_tools);
    }

    #[test]
    fn test_agent_output_to_text() {
        let output = AgentOutput::Text("hello".to_string());
        assert_eq!(output.to_text(), "hello");

        let output = AgentOutput::Json(serde_json::json!({"key": "value"}));
        assert!(output.to_text().contains("key"));
    }
}
