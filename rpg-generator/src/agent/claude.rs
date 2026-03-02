//! Claude agent implementation (optional feature).

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::capabilities::{AgentCapabilities, AgentOutput};
use super::optimizer::{ClaudeOptimizer, PromptOptimizer};
use super::prompt::{PromptFormat, RenderedPrompt};
use crate::error::{GeneratorError, Result};

use super::Agent;

/// Claude agent - extended thinking and artifacts support.
///
/// Uses the Claude CLI with extended thinking mode.
pub struct ClaudeAgent {
    /// Path to claude binary (defaults to "claude").
    binary_path: String,
    /// Model to use (e.g., "claude-3-opus-20240229").
    model: Option<String>,
    /// Enable extended thinking mode.
    thinking: bool,
    /// Working directory for execution.
    working_dir: Option<PathBuf>,
    /// Timeout for execution.
    timeout: Duration,
}

impl ClaudeAgent {
    /// Create a new Claude agent with default settings.
    pub fn new() -> Self {
        Self {
            binary_path: "claude".to_string(),
            model: None,
            thinking: false,
            working_dir: None,
            timeout: Duration::from_secs(300),
        }
    }

    /// Create with custom binary path.
    #[allow(dead_code)]
    pub fn with_binary(mut self, path: impl Into<String>) -> Self {
        self.binary_path = path.into();
        self
    }

    /// Set the model to use.
    #[allow(dead_code)]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Enable extended thinking mode.
    #[allow(dead_code)]
    pub fn with_thinking(mut self) -> Self {
        self.thinking = true;
        self
    }

    /// Set the working directory.
    #[allow(dead_code)]
    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Set the timeout.
    #[allow(dead_code)]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl Default for ClaudeAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for ClaudeAgent {
    fn name(&self) -> &str {
        "claude"
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            supports_extended_thinking: self.thinking,
            supports_docker_isolation: false,
            supports_artifacts: true,
            supports_tools: false,
            supports_multi_agent: false,
            max_context_tokens: Some(200_000),
        }
    }

    fn cli_binary(&self) -> &str {
        &self.binary_path
    }

    async fn execute(&self, prompt: &RenderedPrompt) -> Result<AgentOutput> {
        // Optimize prompt for Claude
        let mut optimized = prompt.clone();
        let optimizer = ClaudeOptimizer::new();
        optimizer.optimize(&mut optimized);

        // Build command
        let mut cmd = Command::new(&self.binary_path);

        // Add print flag for non-interactive mode
        cmd.arg("--print");

        // Add output format
        cmd.arg("--output-format").arg("json");

        // Add model if specified
        if let Some(ref model) = self.model {
            cmd.arg("--model").arg(model);
        }

        // Add thinking flag if enabled
        if self.thinking {
            cmd.arg("--thinking");
        }

        // Set working directory if specified
        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        // Spawn process with piped stdin
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                GeneratorError::ExecutionFailed(format!("Failed to spawn claude: {}", e))
            })?;

        // Write prompt to stdin
        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(optimized.content.as_bytes())
                .await
                .map_err(|e| {
                    GeneratorError::ExecutionFailed(format!("Failed to write to stdin: {}", e))
                })?;
        }

        // Wait for completion with timeout
        let output = tokio::time::timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| GeneratorError::Timeout(self.timeout))?
            .map_err(|e| GeneratorError::ExecutionFailed(format!("Process failed: {}", e)))?;

        // Check exit status
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GeneratorError::ExecutionFailed(stderr.to_string()));
        }

        // Parse output
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Claude outputs JSON in the format:
        // {"content": "...", "thinking": "..." (optional), "artifacts": [...]}
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
            // Extract content
            if let Some(content) = json.get("content").and_then(|c| c.as_str()) {
                // Try to parse content as JSON
                if matches!(prompt.format, PromptFormat::Json | PromptFormat::Structured) {
                    if let Ok(inner_json) = serde_json::from_str(content) {
                        return Ok(AgentOutput::Json(inner_json));
                    }
                }
                return Ok(AgentOutput::Text(content.to_string()));
            }
            // Return full JSON if content not found
            return Ok(AgentOutput::Json(json));
        }

        // Fallback to text
        Ok(AgentOutput::Text(stdout.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_agent_creation() {
        let agent = ClaudeAgent::new();
        assert_eq!(agent.name(), "claude");
        assert!(agent.capabilities().supports_artifacts);
    }

    #[test]
    fn test_claude_agent_with_thinking() {
        let agent = ClaudeAgent::new()
            .with_thinking()
            .with_model("claude-3-opus-20240229");

        assert!(agent.thinking);
        assert!(agent.capabilities().supports_extended_thinking);
        assert_eq!(agent.model, Some("claude-3-opus-20240229".to_string()));
    }

    #[test]
    fn test_claude_agent_timeout() {
        let agent = ClaudeAgent::new().with_timeout(Duration::from_secs(120));

        assert_eq!(agent.timeout, Duration::from_secs(120));
    }
}
