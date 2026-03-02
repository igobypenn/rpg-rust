//! Agent adapter system for coding agent CLIs.
//!
//! This module provides a trait-based abstraction for integrating different
//! coding agent CLIs (opencode, trae-agent, claude) into the RPG generator.
//!
//! # Architecture
//!
//! - **Agent trait**: Core interface for agent execution with capabilities
//! - **Prompt system**: Type-safe prompts with phase-specific contexts
//! - **Feature gates**: Optional CLI implementations via Cargo features
//!
//! # Features
//!
//! - `opencode` (default): OpenCode agent with multi-agent orchestration
//! - `trae`: Trae-agent with Docker isolation and tools
//! - `claude`: Claude CLI with extended thinking and artifacts
//!
//! # Example
//!
//! ```ignore
//! use rpg_generator::agent::{Agent, AgentRegistry, CodeGenContext, Feedback};
//!
//! let registry = AgentRegistry::new();
//! let agent = registry.default_agent();
//!
//! let context = CodeGenContext {
//!     task: task.clone(),
//!     interface: interface.clone(),
//!     feedback: Feedback::default(),
//! };
//!
//! let prompt = CodeGenPrompt.compile(&context, &agent.capabilities());
//! let output = agent.execute(&prompt).await?;
//! ```

mod capabilities;
mod prompt;
mod context;
mod optimizer;

#[cfg(feature = "opencode")]
mod opencode;

#[cfg(feature = "trae")]
mod trae;

#[cfg(feature = "claude")]
mod claude;

pub use capabilities::{AgentCapabilities, AgentOutput, Patch};
pub use prompt::{Feedback, Prompt, PromptFormat, PromptMetadata, RenderedPrompt, TestFailure};
pub use context::{PromptContext, PropertyContext, ImplementationContext, CodeGenContext};

use async_trait::async_trait;
use crate::error::Result;
use std::collections::HashMap;

/// Core trait for coding agent CLIs.
///
/// Each agent implementation wraps a CLI subprocess and provides
/// capability metadata for prompt optimization.
#[async_trait]
pub trait Agent: Send + Sync {
    /// Unique identifier for this agent type.
    fn name(&self) -> &str;
    
    /// Capabilities this agent supports.
    ///
    /// Used by the prompt system to optimize prompts for each agent's strengths.
    fn capabilities(&self) -> AgentCapabilities;
    
    /// Execute a rendered prompt and return structured output.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The rendered prompt ready for execution
    ///
    /// # Returns
    ///
    /// Agent output as text, JSON, or patches.
    async fn execute(&self, prompt: &RenderedPrompt) -> Result<AgentOutput>;
    
    /// Check if the agent CLI is available in PATH.
    fn is_available(&self) -> bool {
        which::which(self.cli_binary()).is_ok()
    }
    
    /// Get the CLI binary name.
    fn cli_binary(&self) -> &str;
}

/// Registry for feature-gated agents.
///
/// Provides runtime access to available agents based on compiled features.
pub struct AgentRegistry {
    agents: HashMap<String, Box<dyn Agent>>,
}

impl AgentRegistry {
    /// Create a new registry with all feature-enabled agents.
    pub fn new() -> Self {
        let mut registry = Self {
            agents: HashMap::new(),
        };
        
        #[cfg(feature = "opencode")]
        registry.register("opencode", Box::new(opencode::OpenCodeAgent::new()));
        
        #[cfg(feature = "trae")]
        registry.register("trae", Box::new(trae::TraeAgent::new()));
        
        #[cfg(feature = "claude")]
        registry.register("claude", Box::new(claude::ClaudeAgent::new()));
        
        registry
    }
    
    /// Register an agent.
    pub fn register(&mut self, name: &str, agent: Box<dyn Agent>) {
        self.agents.insert(name.to_string(), agent);
    }
    
    /// Get an agent by name.
    pub fn get(&self, name: &str) -> Option<&dyn Agent> {
        self.agents.get(name).map(|b| b.as_ref())
    }
    
    /// Get the default agent (opencode if available).
    ///
    /// # Panics
    ///
    /// Panics if no agents are compiled in.
    pub fn default_agent(&self) -> &dyn Agent {
        self.get("opencode")
            .or_else(|| self.agents.values().next().map(|b| b.as_ref()))
            .expect("At least one agent feature must be enabled")
    }
    
    /// Take ownership of the default agent.
    ///
    /// # Panics
    ///
    /// Panics if no agents are compiled in.
    pub fn take_default(&mut self) -> Box<dyn Agent> {
        let key = if self.agents.contains_key("opencode") {
            "opencode".to_string()
        } else {
            self.agents.keys().next()
                .expect("At least one agent feature must be enabled")
                .clone()
        };
        self.agents.remove(&key).expect("agent should exist")
    }
    
    pub fn available_agents(&self) -> Vec<&str> {
        self.agents.keys().map(|s| s.as_str()).collect()
    }
}
impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_default() {
        let registry = AgentRegistry::new();
        // Should not panic if opencode feature is enabled
        #[cfg(feature = "opencode")]
        {
            let agent = registry.default_agent();
            assert_eq!(agent.name(), "opencode");
        }
    }
}
