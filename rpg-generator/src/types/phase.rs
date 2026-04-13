//! Phase tracking types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[derive(Default)]
pub enum Phase {
    #[default]
    Initialization,
    FeaturePlanning,
    ArchitectureDesign,
    CodeGeneration,
    Completed,
}

impl Phase {
    pub fn next(&self) -> Self {
        match self {
            Self::Initialization => Self::FeaturePlanning,
            Self::FeaturePlanning => Self::ArchitectureDesign,
            Self::ArchitectureDesign => Self::CodeGeneration,
            Self::CodeGeneration => Self::Completed,
            Self::Completed => Self::Completed,
        }
    }

    pub fn previous(&self) -> Option<Self> {
        match self {
            Self::Initialization => None,
            Self::FeaturePlanning => Some(Self::Initialization),
            Self::ArchitectureDesign => Some(Self::FeaturePlanning),
            Self::CodeGeneration => Some(Self::ArchitectureDesign),
            Self::Completed => Some(Self::CodeGeneration),
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Initialization => "Initialization",
            Self::FeaturePlanning => "Feature Planning",
            Self::ArchitectureDesign => "Architecture Design",
            Self::CodeGeneration => "Code Generation",
            Self::Completed => "Completed",
        }
    }
}

impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Status tracking for a phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseStatus {
    /// Current phase.
    pub phase: Phase,

    /// When this phase started.
    pub started_at: Option<DateTime<Utc>>,

    /// When this phase completed.
    pub completed_at: Option<DateTime<Utc>>,

    /// Error message if failed.
    pub error: Option<String>,
}

impl PhaseStatus {
    /// Create a new status for the given phase.
    pub fn new(phase: Phase) -> Self {
        Self {
            phase,
            started_at: Some(Utc::now()),
            completed_at: None,
            error: None,
        }
    }

    /// Mark the phase as complete.
    pub fn complete(&mut self) {
        self.completed_at = Some(Utc::now());
    }

    /// Mark the phase as failed.
    pub fn fail(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
        self.completed_at = Some(Utc::now());
    }

    /// Check if the phase is complete.
    pub fn is_complete(&self) -> bool {
        self.completed_at.is_some()
    }

    /// Get the duration of this phase.
    pub fn duration(&self) -> Option<chrono::Duration> {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => Some(end - start),
            _ => None,
        }
    }
}

impl Default for PhaseStatus {
    fn default() -> Self {
        Self::new(Phase::Initialization)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_transitions() {
        let phase = Phase::Initialization;
        assert_eq!(phase.next(), Phase::FeaturePlanning);
        assert_eq!(phase.next().next(), Phase::ArchitectureDesign);
        assert_eq!(phase.next().next().next(), Phase::CodeGeneration);
        assert_eq!(phase.next().next().next().next(), Phase::Completed);
    }

    #[test]
    fn test_phase_previous() {
        assert_eq!(
            Phase::FeaturePlanning.previous(),
            Some(Phase::Initialization)
        );
        assert_eq!(Phase::Initialization.previous(), None);
    }

    #[test]
    fn test_phase_status() {
        let mut status = PhaseStatus::new(Phase::FeaturePlanning);
        assert!(status.started_at.is_some());
        assert!(!status.is_complete());

        status.complete();
        assert!(status.is_complete());
        assert!(status.duration().is_some());
    }
}
