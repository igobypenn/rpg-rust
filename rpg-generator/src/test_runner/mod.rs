//! Test runner abstraction for code generation validation.
//!
//! Provides shell-based test execution with language-specific adapters.

mod shell;

pub use shell::ShellTestRunner;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Result of running tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunResult {
    /// Whether all tests passed.
    pub success: bool,

    /// Number of tests that passed.
    pub passed: usize,

    /// Number of tests that failed.
    pub failed: usize,

    /// Number of tests skipped.
    pub skipped: usize,

    /// Duration in milliseconds.
    pub duration_ms: u64,

    /// Raw output from the test runner.
    pub output: String,

    /// Individual test results.
    pub tests: Vec<IndividualTestResult>,

    /// Path to the project root.
    pub project_path: PathBuf,
}

/// Result of a single test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndividualTestResult {
    /// Test name (fully qualified).
    pub name: String,

    /// Whether this test passed.
    pub passed: bool,

    /// Duration in milliseconds.
    pub duration_ms: u64,

    /// Error message if failed.
    pub error: Option<String>,

    /// Stack trace if available.
    pub stack_trace: Option<String>,
}

/// Configuration for test execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    /// Maximum time to wait for tests (seconds).
    pub timeout_secs: u64,

    /// Whether to run tests in parallel.
    pub parallel: bool,

    /// Maximum parallel test processes.
    pub max_parallel: usize,

    /// Extra environment variables.
    pub env: std::collections::HashMap<String, String>,

    /// Test filter pattern (e.g., "test_name" for --test-threads).
    pub filter: Option<String>,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 120,
            parallel: true,
            max_parallel: 4,
            env: std::collections::HashMap::new(),
            filter: None,
        }
    }
}

impl TestConfig {
    /// Create a new test config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the timeout in seconds.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set whether to run tests in parallel.
    pub fn with_parallel(mut self, parallel: bool) -> Self {
        self.parallel = parallel;
        self
    }

    /// Set a test filter pattern.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }
}

/// Trait for test runners.
pub trait TestRunner: Send + Sync {
    /// Run tests for the project at the given path.
    fn run_tests(
        &self,
        project_path: &std::path::Path,
        config: &TestConfig,
    ) -> crate::Result<TestRunResult>;

    /// Check if the test runner is available.
    fn is_available(&self) -> bool;

    /// Get the name of this test runner.
    fn name(&self) -> &str;
}
