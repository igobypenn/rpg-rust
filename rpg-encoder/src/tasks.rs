//! Implementation task types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationTask {
    pub task_id: String,
    pub file_path: PathBuf,
    pub units: Vec<String>,
    pub unit_code: HashMap<String, String>,
    pub unit_features: HashMap<String, Vec<String>>,
    pub priority: usize,
    pub subtree: String,
    #[serde(default)]
    pub status: TaskStatus,
}

impl ImplementationTask {
    pub fn new(task_id: &str, file_path: PathBuf, subtree: &str) -> Self {
        Self {
            task_id: task_id.to_string(),
            file_path,
            units: Vec::new(),
            unit_code: HashMap::new(),
            unit_features: HashMap::new(),
            priority: 0,
            subtree: subtree.to_string(),
            status: TaskStatus::Pending,
        }
    }

    pub fn add_unit(&mut self, name: &str, code: &str, features: Vec<String>) {
        self.units.push(name.to_string());
        self.unit_code.insert(name.to_string(), code.to_string());
        self.unit_features.insert(name.to_string(), features);
    }

    pub fn with_priority(mut self, priority: usize) -> Self {
        self.priority = priority;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Skipped)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    pub batches: HashMap<String, Vec<ImplementationTask>>,
    pub execution_order: Vec<String>,
}

impl TaskPlan {
    pub fn new() -> Self {
        Self {
            batches: HashMap::new(),
            execution_order: Vec::new(),
        }
    }

    pub fn add_batch(&mut self, component: &str, tasks: Vec<ImplementationTask>) {
        if !self.batches.contains_key(component) {
            self.execution_order.push(component.to_string());
        }
        self.batches.insert(component.to_string(), tasks);
    }

    pub fn all_tasks(&self) -> Vec<&ImplementationTask> {
        self.execution_order
            .iter()
            .flat_map(|component| {
                self.batches
                    .get(component)
                    .map(|v| v.iter())
                    .unwrap_or_default()
            })
            .collect()
    }

    pub fn tasks_for_component(&self, component: &str) -> Vec<&ImplementationTask> {
        self.batches
            .get(component)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    pub fn total_tasks(&self) -> usize {
        self.batches.values().map(|v| v.len()).sum()
    }

    pub fn pending_tasks(&self) -> Vec<&ImplementationTask> {
        self.all_tasks()
            .into_iter()
            .filter(|t| t.status == TaskStatus::Pending)
            .collect()
    }

    pub fn completed_tasks(&self) -> Vec<&ImplementationTask> {
        self.all_tasks()
            .into_iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .collect()
    }

    pub fn failed_tasks(&self) -> Vec<&ImplementationTask> {
        self.all_tasks()
            .into_iter()
            .filter(|t| t.status == TaskStatus::Failed)
            .collect()
    }
}

impl Default for TaskPlan {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let mut task = ImplementationTask::new("task_1", PathBuf::from("src/main.py"), "core");

        task.add_unit("main", "def main(): pass", vec!["entry point".to_string()]);
        task.add_unit("helper", "def helper(): pass", vec!["utility".to_string()]);

        assert_eq!(task.units.len(), 2);
        assert_eq!(task.status, TaskStatus::Pending);
    }

    #[test]
    fn test_task_plan() {
        let mut plan = TaskPlan::new();

        let task1 = ImplementationTask::new("task_1", PathBuf::from("src/a.py"), "core");
        let task2 = ImplementationTask::new("task_2", PathBuf::from("src/b.py"), "ai");

        plan.add_batch("core", vec![task1]);
        plan.add_batch("ai", vec![task2]);

        assert_eq!(plan.total_tasks(), 2);
        assert_eq!(plan.execution_order, vec!["core", "ai"]);
    }

    #[test]
    fn test_task_status_filtering() {
        let mut plan = TaskPlan::new();

        let mut task1 = ImplementationTask::new("task_1", PathBuf::from("src/a.py"), "core");
        task1.status = TaskStatus::Completed;

        let mut task2 = ImplementationTask::new("task_2", PathBuf::from("src/b.py"), "core");
        task2.status = TaskStatus::Failed;

        let task3 = ImplementationTask::new("task_3", PathBuf::from("src/c.py"), "core");

        plan.add_batch("core", vec![task1, task2, task3]);

        assert_eq!(plan.completed_tasks().len(), 1);
        assert_eq!(plan.failed_tasks().len(), 1);
        assert_eq!(plan.pending_tasks().len(), 1);
    }

    #[test]
    fn test_task_serialization() {
        let mut task = ImplementationTask::new("task_1", PathBuf::from("src/main.py"), "core");
        task.add_unit("main", "def main(): pass", vec!["entry".to_string()]);

        let json = serde_json::to_string(&task).unwrap();
        let deserialized: ImplementationTask = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.task_id, task.task_id);
        assert_eq!(deserialized.units, task.units);
    }
}
