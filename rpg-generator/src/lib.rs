//! RPG Generator - Generate codebases from natural language descriptions.
#![allow(clippy::result_large_err)]
//!
//! This crate is a companion to `rpg-encoder` and performs the inverse operation:
//! - **rpg-encoder**: Codebase → RPG (extract structure)
//! - **rpg-generator**: Description → RPG → Code (generate from intent)
//!
//! # Architecture
//!
//! The generator uses a three-phase pipeline:
//! 1. **Phase 1 (Property Level)**: Description → FeatureTree + ComponentPlan
//! 2. **Phase 2 (Implementation Level)**: Components → RepoSkeleton + TaskPlan + Interfaces
//! 3. **Phase 3 (Code Generation)**: Tasks → Generated code via sub-agents (TDD loop)
//! 4. **Phase 4 (Verification)**: Close the loop - verify generated code matches intent
//!
//! # Example
//!
//! ```ignore
//! use rpg_generator::{RpgGenerator, GenerationRequest, TargetLanguage, LlmConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = LlmConfig::new(std::env::var("OPENAI_API_KEY")?);
//!     let generator = RpgGenerator::new(config);
//!     
//!     let request = GenerationRequest::new(
//!         "A REST API for task management",
//!         TargetLanguage::Rust,
//!     );
//!     
//!     let output = generator.generate(request).await?;
//!     println!("Generated {} files with {} tasks completed",
//!         output.total_files(), output.completed_tasks());
//!     Ok(())
//! }
//! ```

pub mod types;
pub mod error;
pub mod checkpoint;
pub mod verification;
pub mod centroid_expander;

// Conditional compilation for phases that require LLM
#[cfg(feature = "llm")]
pub mod llm;
#[cfg(feature = "llm")]
pub mod phases;
#[cfg(feature = "llm")]
pub mod contract;
#[cfg(feature = "llm")]
pub mod test_runner;
#[cfg(feature = "llm")]
pub mod execution;
#[cfg(feature = "llm")]
pub mod pipeline;

// Agent module (feature-gated CLI adapters)
#[cfg(any(feature = "opencode", feature = "trae", feature = "claude"))]
pub mod agent;

// Re-exports for convenience
pub use error::{GeneratorError, Result};
pub use types::{
    GenerationRequest, GenerationPlan, ArchitectureDesign, ExecutionResult,
    Phase, PhaseStatus, TargetLanguage, Constraints,
};

#[cfg(feature = "llm")]
pub use pipeline::{RpgGenerator, GenerationOutput};
#[cfg(feature = "llm")]
pub use llm::LlmConfig;

// Verification module
pub use verification::{GraphVerifier, VerificationResult};

// Centroid Expander
pub use centroid_expander::{CentroidExpander, ExpansionResult, ExpansionConfig, create_planned_graph_from_features};

// Re-export useful types from rpg-encoder
pub use rpg_encoder::{
    FeatureTree, FeatureNode, FlatFeature,
    Component, ComponentPlan,
    RepoSkeleton, SkeletonFile, UnitSkeleton, UnitKind,
    ImplementationTask, TaskPlan, TaskStatus,
    RpgGraph, Node, Edge, EdgeType, NodeCategory, NodeId,
};

/// Prelude for common imports
pub mod prelude {
    pub use crate::error::{GeneratorError, Result};
    pub use crate::types::{
        GenerationRequest, GenerationPlan, ArchitectureDesign, ExecutionResult,
        Phase, PhaseStatus, TargetLanguage,
    };
    pub use crate::checkpoint::{Checkpoint, CheckpointManager};
    pub use crate::verification::{GraphVerifier, VerificationResult};
    pub use crate::centroid_expander::{CentroidExpander, ExpansionResult, ExpansionConfig};
}
