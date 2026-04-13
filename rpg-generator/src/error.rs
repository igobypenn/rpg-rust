//! Error types for the RPG Generator.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

use crate::types::Phase;

pub type Result<T> = std::result::Result<T, GeneratorError>;

/// Top-level error type for the generator.
#[derive(Debug, Error)]
pub enum GeneratorError {
    #[error("Environment not ready: {tool} is required. {install_hint}")]
    Environment { tool: String, install_hint: String },

    #[error("LLM failure in {phase}: {reason}")]
    LlmFailure {
        phase: Phase,
        reason: String,
        raw_output: Option<String>,
    },

    #[error("Contract violation: {violation}")]
    ContractViolation {
        violation: ContractViolation,
        guidance: Option<GuidanceRequest>,
    },

    #[error("Code generation failed for task {task_id}: {error}")]
    CodeGeneration {
        task_id: String,
        error: GenerationError,
    },

    #[error("Tests failed: {passed} passed, {failed} failed")]
    TestFailure {
        passed: usize,
        failed: usize,
        suggestions: Vec<String>,
    },

    #[error("Infrastructure error: {0}")]
    Infrastructure(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Checkpoint error: {0}")]
    Checkpoint(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Missing dependency: {0}")]
    MissingDependency(String),

    #[error("HTTP client error: {0}")]
    Http(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Agent execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Agent execution timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Agent not available: {0}")]
    AgentNotAvailable(String),

    #[error("Failed to parse response: {0}")]
    ParseFailed(String),

    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    #[error("Expansion failed: {0}")]
    ExpansionFailed(String),
}

#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ContractViolation {
    #[error(
        "Feature '{feature}' not found in feature tree (referenced by component '{component}')"
    )]
    MissingFeature { component: String, feature: String },

    #[error("Duplicate name '{name}': {context}")]
    DuplicateName { name: String, context: String },

    #[error("Circular dependency detected: {}", format_cycle(.cycle))]
    CircularDependency { cycle: Vec<String> },

    #[error("Type conflict: '{type_name}' defined in multiple files: {}", .definitions.join(", "))]
    TypeConflict {
        type_name: String,
        definitions: Vec<String>,
    },

    #[error("File conflict: '{path}' claimed by multiple components: {}", .claimants.join(", "))]
    FileConflict {
        path: PathBuf,
        claimants: Vec<String>,
    },

    #[error("Unresolved import in '{module}': {import}")]
    UnresolvedImport { module: String, import: String },

    #[error("Invalid feature tree: {reason}")]
    InvalidFeatureTree { reason: String },

    #[error("Invalid component plan: {reason}")]
    InvalidComponentPlan { reason: String },
}

fn format_cycle(cycle: &[String]) -> String {
    cycle.join(" → ")
}

impl ContractViolation {
    pub fn severity(&self) -> ViolationSeverity {
        match self {
            Self::MissingFeature { .. } | Self::DuplicateName { .. } => {
                ViolationSeverity::AutoFixable { max_retries: 2 }
            }
            Self::CircularDependency { .. } | Self::UnresolvedImport { .. } => {
                ViolationSeverity::RequiresGuidance
            }
            Self::TypeConflict { .. } | Self::FileConflict { .. } => {
                ViolationSeverity::RequiresGuidance
            }
            Self::InvalidFeatureTree { .. } | Self::InvalidComponentPlan { .. } => {
                ViolationSeverity::RequiresHuman
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ViolationSeverity {
    AutoFixable { max_retries: usize },
    RequiresGuidance,
    RequiresHuman,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidanceRequest {
    pub violation: ContractViolation,
    pub explanation: String,
    pub suggested_fixes: Vec<String>,
    pub user_action: UserAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UserAction {
    ModifyPrompt { suggestion: String },
    ChooseOption { options: Vec<String> },
    ManualIntervention { instructions: String },
}

/// Code generation errors.
#[derive(Debug, Clone, Error)]
pub enum GenerationError {
    #[error("Compilation failed: {}", .errors.join("; "))]
    CompilationFailed { errors: Vec<String> },

    #[error("LLM timeout")]
    LlmTimeout,

    #[error("Max retries exceeded")]
    MaxRetriesExceeded,

    #[error("Invalid output: {reason}")]
    InvalidOutput { reason: String },
}

impl GeneratorError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::LlmFailure { .. } | Self::TestFailure { .. } => true,
            Self::ContractViolation { violation, .. } => {
                !matches!(violation.severity(), ViolationSeverity::RequiresHuman)
            }
            Self::CodeGeneration { error, .. } => {
                !matches!(error, GenerationError::MaxRetriesExceeded)
            }
            _ => false,
        }
    }

    pub fn phase(&self) -> Option<Phase> {
        match self {
            Self::LlmFailure { phase, .. } => Some(*phase),
            Self::ContractViolation { .. } => Some(Phase::ArchitectureDesign),
            Self::CodeGeneration { .. } | Self::TestFailure { .. } => Some(Phase::CodeGeneration),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_violation_severity() {
        let missing = ContractViolation::MissingFeature {
            component: "test".to_string(),
            feature: "feat".to_string(),
        };
        assert!(matches!(
            missing.severity(),
            ViolationSeverity::AutoFixable { .. }
        ));

        let circular = ContractViolation::CircularDependency {
            cycle: vec!["A".to_string(), "B".to_string()],
        };
        assert!(matches!(
            circular.severity(),
            ViolationSeverity::RequiresGuidance
        ));
    }

    #[test]
    fn test_error_recoverable() {
        let err = GeneratorError::LlmFailure {
            phase: Phase::FeaturePlanning,
            reason: "timeout".to_string(),
            raw_output: None,
        };
        assert!(err.is_recoverable());
    }
}
