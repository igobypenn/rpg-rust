//! Trae agent implementation (optional feature).

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::capabilities::{AgentCapabilities, AgentOutput, Patch, PatchOperation};
use super::optimizer::{PromptOptimizer, TraeOptimizer};
use super::prompt::{PromptFormat, RenderedPrompt};
use crate::error::{GeneratorError, Result};

use super::Agent;

/// Trae agent - Docker isolation with tool support.
///
/// Uses Docker containers for isolated code generation.
pub struct TraeAgent {
    /// Path to trae binary (defaults to "trae").
    binary_path: String,
    /// Docker image for isolation (if configured).
    docker_image: Option<String>,
    /// Config file path.
    config_path: Option<PathBuf>,
    /// Working directory for execution.
    working_dir: Option<PathBuf>,
    /// Timeout for execution.
    timeout: Duration,
}

impl TraeAgent {
    /// Create a new Trae agent with default settings.
    pub fn new() -> Self {
        Self {
            binary_path: "trae".to_string(),
            docker_image: None,
            config_path: None,
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

    /// Set the Docker image for isolation.
    #[allow(dead_code)]
    pub fn with_docker_image(mut self, image: impl Into<String>) -> Self {
        self.docker_image = Some(image.into());
        self
    }

    /// Set the config file path.
    #[allow(dead_code)]
    pub fn with_config(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into());
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

impl Default for TraeAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for TraeAgent {
    fn name(&self) -> &str {
        "trae"
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            supports_extended_thinking: false,
            supports_docker_isolation: true,
            supports_artifacts: false,
            supports_tools: true,
            supports_multi_agent: false,
            max_context_tokens: Some(200_000),
        }
    }

    fn cli_binary(&self) -> &str {
        &self.binary_path
    }

    async fn execute(&self, prompt: &RenderedPrompt) -> Result<AgentOutput> {
        // Optimize prompt for Trae
        let mut optimized = prompt.clone();
        let optimizer = TraeOptimizer::new();
        optimizer.optimize(&mut optimized);

        // Build command
        let mut cmd = Command::new(&self.binary_path);

        // Add config if specified
        if let Some(ref config) = self.config_path {
            cmd.arg("--config").arg(config);
        }

        // Add Docker image if specified
        if let Some(ref image) = self.docker_image {
            cmd.arg("--docker").arg(image);
        }

        // Set working directory if specified
        if let Some(ref dir) = self.working_dir {
            cmd.current_dir(dir);
        }

        // Add task argument
        cmd.arg("--task");

        // Spawn process with piped stdin
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                GeneratorError::ExecutionFailed(format!("Failed to spawn trae: {}", e))
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

        // Parse output - trae returns patches
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Try to parse as patches
        let patches = parse_patches(&stdout);
        if !patches.is_empty() {
            return Ok(AgentOutput::Patches(patches));
        }

        // Try JSON
        if matches!(
            prompt.format,
            PromptFormat::Json | PromptFormat::Structured
        ) {
            if let Ok(json) = serde_json::from_str(&stdout) {
                return Ok(AgentOutput::Json(json));
            }
        }

        // Default to text
        Ok(AgentOutput::Text(stdout.to_string()))
    }
}

/// Parse patches from trae output.
fn parse_patches(output: &str) -> Vec<Patch> {
    let mut patches = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_content = String::new();

    for line in output.lines() {
        // Look for diff headers like "--- a/path/to/file.rs"
        if line.starts_with("--- a/") {
            // Save previous patch
            if let Some(path) = current_path.take() {
                if !current_content.is_empty() {
                    patches.push(Patch {
                        path: PathBuf::from(path),
                        content: std::mem::take(&mut current_content),
                        operation: PatchOperation::Modify,
                    });
                }
            }
            current_path = Some(line.trim_start_matches("--- a/").to_string());
        } else if line.starts_with("+++ /dev/null") {
            // File deletion marker
            if let Some(path) = current_path.take() {
                patches.push(Patch {
                    path: PathBuf::from(path),
                    content: String::new(),
                    operation: PatchOperation::Delete,
                });
            }
        } else if let Some(ref _path) = current_path {
            // Accumulate content
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    // Save last patch
    if let Some(path) = current_path {
        if !current_content.is_empty() {
            patches.push(Patch {
                path: PathBuf::from(path),
                content: current_content,
                operation: PatchOperation::Modify,
            });
        }
    }

    patches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trae_agent_creation() {
        let agent = TraeAgent::new();
        assert_eq!(agent.name(), "trae");
        assert!(agent.capabilities().supports_docker_isolation);
        assert!(agent.capabilities().supports_tools);
    }

    #[test]
    fn test_trae_agent_with_docker() {
        let agent = TraeAgent::new().with_docker_image("ghcr.io/trae/agent:latest");

        assert_eq!(
            agent.docker_image,
            Some("ghcr.io/trae/agent:latest".to_string())
        );
    }

    #[test]
    fn test_parse_patches() {
        let output = r#"--- a/src/main.rs
+++ b/src/main.rs
@@ -1,5 +1,6 @@
 fn main() {
-    println!("Hello");
+    println!("Hello, World!");
 }
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,4 @@
 pub fn add(a: i32, b: i32) -> i32 {
+    // Add two numbers
     a + b
 }
"#;
        let patches = parse_patches(output);
        assert_eq!(patches.len(), 2);
    }
}
