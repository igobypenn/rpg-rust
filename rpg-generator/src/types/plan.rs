//! Generation plan and result types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use super::{GenerationRequest, PhaseStatus};

// Re-export planning types from rpg-encoder
pub use rpg_encoder::{ComponentPlan, FeatureTree, RepoSkeleton, TaskPlan, TaskStatus, UnitKind};

/// The output of Phase 1: Feature Planning.
///
/// Contains the parsed features and component breakdown from the description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationPlan {
    /// Unique identifier for this plan.
    pub id: Uuid,

    /// The original request.
    pub request: GenerationRequest,

    /// The extracted feature tree.
    pub feature_tree: FeatureTree,

    /// The component plan (features grouped into components).
    pub component_plan: ComponentPlan,

    /// Current status of this plan.
    pub status: PhaseStatus,

    /// Planned RPG graph (from CentroidExpander).
    /// This encodes the intended architecture from features.
    #[serde(default)]
    pub planned_rpg: Option<rpg_encoder::RpgGraph>,
}

impl GenerationPlan {
    /// Create a new generation plan.
    pub fn new(
        request: GenerationRequest,
        feature_tree: FeatureTree,
        component_plan: ComponentPlan,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            request,
            feature_tree,
            component_plan,
            status: PhaseStatus::new(super::Phase::FeaturePlanning),
            planned_rpg: None,
        }
    }

    /// Mark the plan as complete.
    pub fn complete(&mut self) {
        self.status.complete();
    }

    /// Set the planned RPG graph.
    pub fn set_planned_rpg(&mut self, graph: rpg_encoder::RpgGraph) {
        self.planned_rpg = Some(graph);
    }
}
/// File interface specification for Phase 2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInterface {
    /// Path to the file.
    pub path: PathBuf,

    /// Units (functions, classes) in this file.
    pub units: Vec<UnitInterface>,

    /// Required imports.
    pub imports: Vec<String>,
}

/// Interface for a single code unit (function, class, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitInterface {
    /// Name of the unit.
    pub name: String,

    /// Kind of unit (function, class, etc.).
    pub kind: UnitKind,

    /// Function/method signature.
    pub signature: Option<String>,

    /// Documentation string.
    pub docstring: Option<String>,

    /// Features this unit implements.
    pub features: Vec<String>,
}

/// Type definition for TypeRegistry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDefinition {
    /// Name of the type.
    pub name: String,

    /// Source file where the type is defined.
    pub source_file: PathBuf,

    /// Kind of type (struct, enum, class, etc.).
    pub kind: String,

    /// Fields or variants (if applicable).
    pub fields: Vec<TypeField>,
}

/// Field in a type definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeField {
    /// Field name.
    pub name: String,

    /// Field type.
    pub type_name: String,
}

/// The output of Phase 2: Implementation Planning.
///
/// Contains the file skeleton, interfaces, and task breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureDesign {
    /// ID of the parent GenerationPlan.
    pub plan_id: Uuid,

    /// The file skeleton (directory structure).
    pub skeleton: RepoSkeleton,

    /// The task plan (implementation tasks).
    pub task_plan: TaskPlan,

    /// File interfaces (signatures, docstrings).
    pub interfaces: HashMap<PathBuf, FileInterface>,

    /// Type registry for consistency checking.
    pub type_registry: HashMap<String, TypeDefinition>,

    /// Current status.
    pub status: PhaseStatus,
}

impl ArchitectureDesign {
    /// Create a new architecture design.
    pub fn new(plan_id: Uuid, skeleton: RepoSkeleton, task_plan: TaskPlan) -> Self {
        Self {
            plan_id,
            skeleton,
            task_plan,
            interfaces: HashMap::new(),
            type_registry: HashMap::new(),
            status: PhaseStatus::new(super::Phase::ArchitectureDesign),
        }
    }

    /// Add a file interface.
    pub fn add_interface(&mut self, interface: FileInterface) {
        self.interfaces.insert(interface.path.clone(), interface);
    }

    /// Register a type definition.
    pub fn register_type(&mut self, def: TypeDefinition) -> Result<(), String> {
        if let Some(existing) = self.type_registry.get(&def.name) {
            if existing.source_file != def.source_file {
                return Err(format!(
                    "Type '{}' defined in multiple files: {} and {}",
                    def.name,
                    existing.source_file.display(),
                    def.source_file.display()
                ));
            }
        }
        self.type_registry.insert(def.name.clone(), def);
        Ok(())
    }

    /// Mark the design as complete.
    pub fn complete(&mut self) {
        self.status.complete();
    }
}

/// Task outcome for tracking execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutcome {
    /// Task ID.
    pub task_id: String,

    /// Final status.
    pub status: TaskStatus,

    /// Number of iterations to complete.
    pub iterations: usize,

    /// Test pass rate (0.0 - 1.0).
    pub test_pass_rate: f32,

    /// When the task was completed.
    pub generated_at: DateTime<Utc>,
}

/// Test result for a single test run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Task ID this result is for.
    pub task_id: String,

    /// Number of passed tests.
    pub passed: usize,

    /// Number of failed tests.
    pub failed: usize,

    /// Test errors.
    pub errors: Vec<TestError>,
}

/// A test error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestError {
    /// Test name.
    pub test_name: String,

    /// Error message.
    pub message: String,

    /// Line number (if available).
    pub line: Option<usize>,

    /// Stack trace (if available).
    pub stack_trace: Option<String>,
}

/// The output of Phase 3: Code Generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// ID of the parent ArchitectureDesign.
    pub design_id: Uuid,

    /// Outcome for each task.
    pub task_outcomes: HashMap<String, TaskOutcome>,

    /// Generated code by file path.
    pub generated_code: HashMap<PathBuf, String>,

    /// Test results.
    pub test_results: Vec<TestResult>,

    /// Final RPG graph of the generated codebase.
    #[serde(default)]
    pub final_graph: Option<rpg_encoder::RpgGraph>,

    /// Current status.
    pub status: PhaseStatus,
}

impl ExecutionResult {
    /// Create a new execution result.
    pub fn new(design_id: Uuid) -> Self {
        Self {
            design_id,
            task_outcomes: HashMap::new(),
            generated_code: HashMap::new(),
            test_results: Vec::new(),
            final_graph: None,
            status: PhaseStatus::new(super::Phase::CodeGeneration),
        }
    }

    /// Add a generated file.
    pub fn add_file(&mut self, path: PathBuf, code: String) {
        self.generated_code.insert(path, code);
    }

    /// Add a task outcome.
    pub fn add_outcome(&mut self, outcome: TaskOutcome) {
        self.task_outcomes.insert(outcome.task_id.clone(), outcome);
    }

    /// Get total number of generated files.
    pub fn file_count(&self) -> usize {
        self.generated_code.len()
    }

    /// Get count of completed tasks.
    pub fn completed_count(&self) -> usize {
        self.task_outcomes
            .values()
            .filter(|o| o.status == TaskStatus::Completed)
            .count()
    }

    /// Get count of failed tasks.
    pub fn failed_count(&self) -> usize {
        self.task_outcomes
            .values()
            .filter(|o| o.status == TaskStatus::Failed)
            .count()
    }

    /// Mark the result as complete.
    pub fn complete(&mut self) {
        self.status.complete();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_result() {
        let mut result = ExecutionResult::new(Uuid::new_v4());

        result.add_file(PathBuf::from("src/main.rs"), "fn main() {}".to_string());
        result.add_file(PathBuf::from("src/lib.rs"), "pub fn add() {}".to_string());

        assert_eq!(result.file_count(), 2);
    }

    #[test]
    fn test_type_registration() {
        let mut design = ArchitectureDesign::new(
            Uuid::new_v4(),
            RepoSkeleton::new(PathBuf::from("src"), "rust"),
            TaskPlan::new(),
        );

        let type_def = TypeDefinition {
            name: "User".to_string(),
            source_file: PathBuf::from("src/models/user.rs"),
            kind: "struct".to_string(),
            fields: vec![],
        };

        assert!(design.register_type(type_def).is_ok());

        // Duplicate from same file should succeed
        let type_def2 = TypeDefinition {
            name: "User".to_string(),
            source_file: PathBuf::from("src/models/user.rs"),
            kind: "struct".to_string(),
            fields: vec![],
        };
        assert!(design.register_type(type_def2).is_ok());

        // Duplicate from different file should fail
        let type_def3 = TypeDefinition {
            name: "User".to_string(),
            source_file: PathBuf::from("src/types.rs"),
            kind: "struct".to_string(),
            fields: vec![],
        };
        assert!(design.register_type(type_def3).is_err());
    }
}
