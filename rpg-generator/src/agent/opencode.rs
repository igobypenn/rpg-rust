//! OpenCode agent implementation (default feature).

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use tokio::process::Command;

use super::capabilities::{AgentCapabilities, AgentOutput};
use super::optimizer::{OpenCodeOptimizer, PromptOptimizer};
use super::prompt::{PromptFormat, RenderedPrompt};
use crate::error::{GeneratorError, Result};

use super::Agent;

/// OpenCode agent - multi-agent orchestration with hooks.
///
/// This is the primary/default agent for RPG generator.
pub struct OpenCodeAgent {
    /// Path to opencode binary (defaults to "opencode").
    binary_path: String,
    /// Agent type to use (e.g., "build", "oracle").
    agent_type: Option<String>,
    /// Working directory for execution.
    working_dir: Option<PathBuf>,
    /// Timeout for execution.
    timeout: Duration,
    /// Model to use (e.g., "anthropic/claude-3.5-sonnet").
    model: Option<String>,
}

impl OpenCodeAgent {
    /// Create a new OpenCode agent with default settings.
    pub fn new() -> Self {
        Self {
            binary_path: "opencode".to_string(),
            agent_type: None,
            working_dir: None,
            timeout: Duration::from_secs(600), // 10 minutes for complex generations
            model: None,
        }
    }

    /// Create with custom binary path.
    #[allow(dead_code)]
    pub fn with_binary(mut self, path: impl Into<String>) -> Self {
        self.binary_path = path.into();
        self
    }

    /// Set the agent type (e.g., "build", "oracle").
    #[allow(dead_code)]
    pub fn with_agent_type(mut self, agent_type: impl Into<String>) -> Self {
        self.agent_type = Some(agent_type.into());
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

    /// Set the model to use.
    #[allow(dead_code)]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

impl Default for OpenCodeAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for OpenCodeAgent {
    fn name(&self) -> &str {
        "opencode"
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            supports_extended_thinking: false,
            supports_docker_isolation: false,
            supports_artifacts: false,
            supports_tools: false,
            supports_multi_agent: true,
            max_context_tokens: Some(128_000),
        }
    }

    fn cli_binary(&self) -> &str {
        &self.binary_path
    }

    async fn execute(&self, prompt: &RenderedPrompt) -> Result<AgentOutput> {
        // Optimize prompt for OpenCode
        let mut optimized = prompt.clone();
        let optimizer = OpenCodeOptimizer::new();
        optimizer.optimize(&mut optimized);

        // Log the request
        tracing::info!(
            "=== OpenCode Agent Request ===\n\
             Phase: {:?}\n\
             Format: {:?}\n\
             Prompt length: {} chars\n\
             Prompt preview: {}...\n\
             ==============================",
            prompt.metadata.phase,
            prompt.format,
            optimized.content.len(),
            &optimized.content[..optimized.content.len().min(500)]
        );

        // Build command: opencode run "message" --format json
        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("run");
        cmd.arg(&optimized.content);
        cmd.arg("--format").arg("json");

        // Add agent type if specified
        if let Some(ref agent_type) = self.agent_type {
            cmd.arg("--agent").arg(agent_type);
            tracing::info!("Using agent type: {}", agent_type);
        }

        // Add model if specified
        if let Some(ref model) = self.model {
            cmd.arg("--model").arg(model);
            tracing::info!("Using model: {}", model);
        }

        // Set working directory if specified
        if let Some(ref dir) = self.working_dir {
            cmd.arg("--dir").arg(dir);
            tracing::info!("Working directory: {}", dir.display());
        }

        // Enable logging
        cmd.arg("--print-logs");
        cmd.arg("--log-level").arg("INFO");

        tracing::info!("Spawning opencode with timeout: {:?}", self.timeout);

        // Spawn process
        let start_time = std::time::Instant::now();

        let output = cmd.output().await.map_err(|e| {
            tracing::error!("Failed to spawn opencode: {}", e);
            GeneratorError::ExecutionFailed(format!("Failed to spawn opencode: {}", e))
        })?;

        let elapsed = start_time.elapsed();
        tracing::info!("OpenCode completed in {:.2}s", elapsed.as_secs_f64());

        // Check exit status
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            tracing::error!("OpenCode failed with status: {}", output.status);
            tracing::error!("stderr: {}", stderr);
            tracing::error!("stdout: {}", stdout);
            return Err(GeneratorError::ExecutionFailed(format!(
                "OpenCode failed (exit {}): {}",
                output.status, stderr
            )));
        }

        // Parse output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Log stderr for debugging (contains progress info)
        if !stderr.is_empty() {
            tracing::debug!("OpenCode stderr: {}", stderr);
        }

        tracing::debug!("OpenCode stdout length: {} bytes", stdout.len());

        // Try to parse as JSON if format expects structured output
        if matches!(prompt.format, PromptFormat::Json | PromptFormat::Structured) {
            // The output might contain multiple JSON lines (SSE format)
            // Find the last valid JSON object
            for line in stdout.lines().rev() {
                if line.starts_with('{') || line.starts_with('[') {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                        tracing::info!("Parsed JSON response successfully");
                        return Ok(AgentOutput::Json(json));
                    }
                }
            }

            // Try parsing the entire output as JSON
            if let Ok(json) = serde_json::from_str(&stdout) {
                tracing::info!("Parsed full stdout as JSON successfully");
                return Ok(AgentOutput::Json(json));
            }

            tracing::warn!("Expected JSON output but failed to parse. Returning as text.");
        }

        // Default to text output
        tracing::info!("Returning text output ({} bytes)", stdout.len());
        Ok(AgentOutput::Text(stdout.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opencode_agent_creation() {
        let agent = OpenCodeAgent::new();
        assert_eq!(agent.name(), "opencode");
        assert!(agent.capabilities().supports_multi_agent);
    }

    #[test]
    fn test_opencode_agent_with_options() {
        let agent = OpenCodeAgent::new()
            .with_agent_type("build")
            .with_timeout(Duration::from_secs(60));

        assert_eq!(agent.agent_type, Some("build".to_string()));
        assert_eq!(agent.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_opencode_agent_is_available() {
        let agent = OpenCodeAgent::new();
        let available = agent.is_available();
        println!("OpenCode CLI available: {}", available);
    }

    #[tokio::test]
    async fn test_opencode_prompt_rendering() {
        use crate::agent::{CodeGenContext, Feedback};
        use crate::types::FileInterface;

        let task = crate::ImplementationTask::new(
            "test_task",
            PathBuf::from("src/test.rs"),
            "test_component",
        );

        let interface = FileInterface {
            path: PathBuf::from("src/test.rs"),
            units: vec![],
            imports: vec![],
        };

        let ctx = CodeGenContext {
            task,
            interface,
            feedback: Feedback::new(),
        };

        let agent = OpenCodeAgent::new();
        let prompt = ctx.compile(&agent.capabilities());

        assert!(prompt.content.contains("src/test.rs"));
        assert!(prompt.content.contains("test_component"));
    }
}
