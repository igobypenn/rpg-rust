//! Checkpoint manager implementation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::{GeneratorError, Result};
use crate::types::{ArchitectureDesign, ExecutionResult, GenerationPlan, Phase};

pub type CheckpointId = Uuid;

/// Serializable checkpoint for resume support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: CheckpointId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub description: String,
    pub language: String,

    pub generation_plan: Option<GenerationPlan>,
    pub architecture_design: Option<ArchitectureDesign>,
    pub execution_result: Option<ExecutionResult>,

    pub current_phase: Phase,
    pub completed_phases: Vec<Phase>,
    pub task_progress: HashMap<String, TaskProgress>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgress {
    pub status: String,
    pub attempts: usize,
    pub last_attempt: Option<DateTime<Utc>>,
}

impl Checkpoint {
    pub fn new(description: impl Into<String>, language: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            description: description.into(),
            language: language.into(),
            generation_plan: None,
            architecture_design: None,
            execution_result: None,
            current_phase: Phase::Initialization,
            completed_phases: Vec::new(),
            task_progress: HashMap::new(),
        }
    }

    pub fn update_timestamp(&mut self) {
        self.updated_at = Utc::now();
    }

    pub fn advance_phase(&mut self, new_phase: Phase) {
        if self.current_phase != new_phase {
            self.completed_phases.push(self.current_phase);
            self.current_phase = new_phase;
            self.update_timestamp();
        }
    }

    pub fn set_generation_plan(&mut self, plan: GenerationPlan) {
        self.generation_plan = Some(plan);
        self.update_timestamp();
    }

    pub fn set_architecture_design(&mut self, design: ArchitectureDesign) {
        self.architecture_design = Some(design);
        self.update_timestamp();
    }

    pub fn set_execution_result(&mut self, result: ExecutionResult) {
        self.execution_result = Some(result);
        self.update_timestamp();
    }

    pub fn update_task_progress(&mut self, task_id: String, status: String, attempts: usize) {
        self.task_progress.insert(
            task_id,
            TaskProgress {
                status,
                attempts,
                last_attempt: Some(Utc::now()),
            },
        );
        self.update_timestamp();
    }

    pub fn last_successful_phase(&self) -> Phase {
        self.current_phase
    }

    pub fn is_complete(&self) -> bool {
        matches!(self.current_phase, Phase::Completed)
    }
}

pub struct CheckpointManager {
    checkpoint_dir: PathBuf,
    current: Checkpoint,
    auto_save: bool,
}

impl CheckpointManager {
    pub fn new(checkpoint_dir: PathBuf, description: String, language: String) -> Self {
        Self {
            checkpoint_dir,
            current: Checkpoint::new(description, language),
            auto_save: true,
        }
    }

    pub fn with_auto_save(mut self, auto_save: bool) -> Self {
        self.auto_save = auto_save;
        self
    }

    pub fn load(path: &Path) -> Result<Self> {
        let checkpoint_file = path.join("checkpoint.json");
        let content = std::fs::read_to_string(&checkpoint_file)
            .map_err(|e| GeneratorError::Checkpoint(format!("Failed to read checkpoint: {}", e)))?;

        let checkpoint: Checkpoint = serde_json::from_str(&content).map_err(|e| {
            GeneratorError::Checkpoint(format!("Failed to parse checkpoint: {}", e))
        })?;

        Ok(Self {
            checkpoint_dir: path.to_path_buf(),
            current: checkpoint,
            auto_save: true,
        })
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(&self.checkpoint_dir).map_err(|e| {
            GeneratorError::Checkpoint(format!("Failed to create checkpoint dir: {}", e))
        })?;

        let checkpoint_file = self.checkpoint_dir.join("checkpoint.json");
        let content = serde_json::to_string_pretty(&self.current).map_err(|e| {
            GeneratorError::Checkpoint(format!("Failed to serialize checkpoint: {}", e))
        })?;

        std::fs::write(&checkpoint_file, content).map_err(|e| {
            GeneratorError::Checkpoint(format!("Failed to write checkpoint: {}", e))
        })?;

        Ok(())
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)
            .map_err(|e| GeneratorError::Checkpoint(format!("Failed to create dir: {}", e)))?;

        let checkpoint_file = path.join("checkpoint.json");
        let content = serde_json::to_string_pretty(&self.current)
            .map_err(|e| GeneratorError::Checkpoint(format!("Failed to serialize: {}", e)))?;

        std::fs::write(&checkpoint_file, content)
            .map_err(|e| GeneratorError::Checkpoint(format!("Failed to write: {}", e)))?;

        Ok(())
    }

    pub fn checkpoint(&self) -> &Checkpoint {
        &self.current
    }

    pub fn checkpoint_mut(&mut self) -> &mut Checkpoint {
        &mut self.current
    }

    pub fn advance_phase(&mut self, phase: Phase) -> Result<()> {
        self.current.advance_phase(phase);
        if self.auto_save {
            self.save()?;
        }
        Ok(())
    }

    pub fn set_generation_plan(&mut self, plan: GenerationPlan) -> Result<()> {
        self.current.set_generation_plan(plan);
        if self.auto_save {
            self.save()?;
        }
        Ok(())
    }

    pub fn set_architecture_design(&mut self, design: ArchitectureDesign) -> Result<()> {
        self.current.set_architecture_design(design);
        if self.auto_save {
            self.save()?;
        }
        Ok(())
    }

    pub fn set_execution_result(&mut self, result: ExecutionResult) -> Result<()> {
        self.current.set_execution_result(result);
        if self.auto_save {
            self.save()?;
        }
        Ok(())
    }

    pub fn update_task(&mut self, task_id: String, status: String, attempts: usize) -> Result<()> {
        self.current.update_task_progress(task_id, status, attempts);
        if self.auto_save {
            self.save()?;
        }
        Ok(())
    }

    pub fn current_phase(&self) -> Phase {
        self.current.current_phase
    }

    pub fn id(&self) -> CheckpointId {
        self.current.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;


    #[test]
    fn test_checkpoint_creation() {
        let checkpoint = Checkpoint::new("Test project", "rust");
        assert_eq!(checkpoint.description, "Test project");
        assert_eq!(checkpoint.current_phase, Phase::Initialization);
    }

    #[test]
    fn test_checkpoint_phase_advancement() {
        let mut checkpoint = Checkpoint::new("Test", "rust");
        checkpoint.advance_phase(Phase::FeaturePlanning);
        assert_eq!(checkpoint.current_phase, Phase::FeaturePlanning);
        assert!(checkpoint.completed_phases.contains(&Phase::Initialization));
    }

    #[test]
    fn test_checkpoint_manager_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        let mut manager =
            CheckpointManager::new(path.clone(), "Test project".to_string(), "rust".to_string());
        manager.advance_phase(Phase::FeaturePlanning).unwrap();

        let id = manager.id();

        let loaded = CheckpointManager::load(&path).unwrap();
        assert_eq!(loaded.id(), id);
        assert_eq!(loaded.current_phase(), Phase::FeaturePlanning);
    }
}
