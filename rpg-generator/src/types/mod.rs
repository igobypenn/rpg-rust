//! Core types for the RPG Generator pipeline.

mod request;
pub mod plan;
mod phase;

pub use request::{GenerationRequest, TargetLanguage, Constraints};
pub use plan::{
    GenerationPlan, ArchitectureDesign, ExecutionResult,
    TaskOutcome, TestResult, TestError,
};
pub use phase::{Phase, PhaseStatus, PhaseType};

// Re-export validation types
pub use rpg_encoder::{ValidationResult, ValidationIssue};

pub use plan::{FileInterface, UnitInterface, TypeDefinition, TypeField};
