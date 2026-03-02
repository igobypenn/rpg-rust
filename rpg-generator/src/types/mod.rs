//! Core types for the RPG Generator pipeline.

mod phase;
pub mod plan;
mod request;

pub use phase::{Phase, PhaseStatus, PhaseType};
pub use plan::{
    ArchitectureDesign, ExecutionResult, GenerationPlan, TaskOutcome, TestError, TestResult,
};
pub use request::{Constraints, GenerationRequest, TargetLanguage};

// Re-export validation types
pub use rpg_encoder::{ValidationIssue, ValidationResult};

pub use plan::{FileInterface, TypeDefinition, TypeField, UnitInterface};
