//! Shell-based test runner with language-specific adapters.

use std::process::{Command, Stdio};
use std::time::Instant;

use super::{IndividualTestResult, TestConfig, TestRunResult, TestRunner};
use crate::error::GeneratorError;
use crate::types::TargetLanguage;

/// Shell-based test runner that uses language-specific commands.
pub struct ShellTestRunner {
    language: TargetLanguage,
}

impl ShellTestRunner {
    /// Create a new shell test runner for the given language.
    pub fn new(language: TargetLanguage) -> Self {
        Self { language }
    }

    /// Get the test command for the current language.
    fn test_command(&self, config: &TestConfig) -> Vec<String> {
        match self.language {
            TargetLanguage::Rust => {
                let mut cmd = vec!["cargo", "test", "--no-fail-fast"];
                if !config.parallel {
                    cmd.push("-- --test-threads=1");
                }
                if let Some(ref filter) = config.filter {
                    cmd.push("--");
                    cmd.push(filter);
                }
                cmd.iter().map(|s| s.to_string()).collect()
            }
            TargetLanguage::Python => {
                let mut cmd = vec!["python", "-m", "pytest", "-v"];
                if !config.parallel {
                    cmd.push("-n0");
                }
                if let Some(ref filter) = config.filter {
                    cmd.push("-k");
                    cmd.push(filter);
                }
                cmd.iter().map(|s| s.to_string()).collect()
            }
            TargetLanguage::TypeScript | TargetLanguage::JavaScript => {
                ["npm", "test"].iter().map(|s| s.to_string()).collect()
            }
            TargetLanguage::Go => {
                let mut cmd = vec!["go", "test", "-v", "./..."];
                if let Some(ref filter) = config.filter {
                    cmd.push("-run");
                    cmd.push(filter);
                }
                cmd.iter().map(|s| s.to_string()).collect()
            }
            TargetLanguage::Java => ["mvn", "test"].iter().map(|s| s.to_string()).collect(),
            _ => ["echo", "No test runner for this language"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }

    /// Parse test output based on language.
    fn parse_output(&self, output: &str) -> (usize, usize, usize, Vec<IndividualTestResult>) {
        match self.language {
            TargetLanguage::Rust => self.parse_rust_output(output),
            TargetLanguage::Python => self.parse_pytest_output(output),
            TargetLanguage::Go => self.parse_go_output(output),
            _ => self.parse_generic_output(output),
        }
    }

    fn parse_rust_output(&self, output: &str) -> (usize, usize, usize, Vec<IndividualTestResult>) {
        let mut passed = 0usize;
        let mut failed = 0usize;
        let mut tests = Vec::new();

        for line in output.lines() {
            if line.contains("test result:") {
                if let Some(nums) = Self::extract_numbers(line) {
                    passed = nums.0;
                    failed = nums.1;
                }
            } else if line.contains(" ... ok") {
                let name = line
                    .replace("test ", "")
                    .replace(" ... ok", "")
                    .trim()
                    .to_string();
                tests.push(IndividualTestResult {
                    name,
                    passed: true,
                    duration_ms: 0,
                    error: None,
                    stack_trace: None,
                });
            } else if line.contains(" ... FAILED") {
                let name = line
                    .replace("test ", "")
                    .replace(" ... FAILED", "")
                    .trim()
                    .to_string();
                tests.push(IndividualTestResult {
                    name,
                    passed: false,
                    duration_ms: 0,
                    error: Some("Test failed".to_string()),
                    stack_trace: None,
                });
            }
        }

        let total = passed + failed;
        (passed, failed, total.saturating_sub(passed + failed), tests)
    }

    fn parse_pytest_output(
        &self,
        output: &str,
    ) -> (usize, usize, usize, Vec<IndividualTestResult>) {
        let mut passed = 0usize;
        let mut failed = 0usize;
        let mut tests = Vec::new();

        for line in output.lines() {
            if line.starts_with('=') && line.contains("passed") {
                if let Some(nums) = Self::extract_numbers(line) {
                    passed = nums.0;
                    failed = nums.1;
                }
            } else if line.contains(" PASSED") {
                let name = line
                    .split(" PASSED")
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                tests.push(IndividualTestResult {
                    name,
                    passed: true,
                    duration_ms: 0,
                    error: None,
                    stack_trace: None,
                });
            } else if line.contains(" FAILED") {
                let name = line
                    .split(" FAILED")
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                tests.push(IndividualTestResult {
                    name,
                    passed: false,
                    duration_ms: 0,
                    error: Some("Test failed".to_string()),
                    stack_trace: None,
                });
            }
        }

        (passed, failed, 0, tests)
    }

    fn parse_go_output(&self, output: &str) -> (usize, usize, usize, Vec<IndividualTestResult>) {
        let mut passed = 0usize;
        let mut failed = 0usize;
        let mut tests = Vec::new();

        for line in output.lines() {
            if line.starts_with("PASS") || line.starts_with("FAIL") {
                continue;
            }
            if line.starts_with("=== RUN ") {
                // Test started
            } else if line.starts_with("    --- PASS: ") {
                let name = line.replace("    --- PASS: ", "").trim().to_string();
                passed += 1;
                tests.push(IndividualTestResult {
                    name,
                    passed: true,
                    duration_ms: 0,
                    error: None,
                    stack_trace: None,
                });
            } else if line.starts_with("    --- FAIL: ") {
                let name = line.replace("    --- FAIL: ", "").trim().to_string();
                failed += 1;
                tests.push(IndividualTestResult {
                    name,
                    passed: false,
                    duration_ms: 0,
                    error: Some("Test failed".to_string()),
                    stack_trace: None,
                });
            }
        }

        (passed, failed, 0, tests)
    }

    fn parse_generic_output(
        &self,
        output: &str,
    ) -> (usize, usize, usize, Vec<IndividualTestResult>) {
        // Generic parsing - look for common patterns
        let passed = output.matches("passed").count() + output.matches("PASS").count();
        let failed = output.matches("failed").count() + output.matches("FAIL").count();
        (passed, failed, 0, Vec::new())
    }

    fn extract_numbers(s: &str) -> Option<(usize, usize)> {
        // Extract "X passed" and "Y failed" from a string
        let passed = Self::extract_number_before(s, "passed")?;
        let failed = Self::extract_number_before(s, "failed").unwrap_or(0);
        Some((passed, failed))
    }

    fn extract_number_before(s: &str, keyword: &str) -> Option<usize> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        for (i, part) in parts.iter().enumerate() {
            let part_clean = part.trim_matches(',').trim_matches('.').trim_matches(';');
            if part_clean == keyword && i > 0 {
                return parts[i - 1].parse().ok();
            }
        }
        None
    }
}

impl TestRunner for ShellTestRunner {
    fn run_tests(
        &self,
        project_path: &std::path::Path,
        config: &TestConfig,
    ) -> crate::Result<TestRunResult> {
        let cmd_parts = self.test_command(config);
        if cmd_parts.is_empty() {
            return Err(GeneratorError::NotImplemented(
                "No test runner for this language".to_string(),
            ));
        }

        let (program, args) = cmd_parts
            .split_first()
            .map(|(p, a)| (p.as_str(), a.iter().map(|s| s.as_str()).collect::<Vec<_>>()))
            .unwrap_or(("", vec![]));

        let start = Instant::now();

        let mut command = Command::new(program);
        command
            .args(&args)
            .current_dir(project_path)
            .envs(&config.env)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = command.output().map_err(|e| GeneratorError::Environment {
            tool: program.to_string(),
            install_hint: format!("Failed to execute test command: {}", e),
        })?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined_output = format!("{}\n{}", stdout, stderr);

        let (passed, failed, skipped, tests) = self.parse_output(&combined_output);

        Ok(TestRunResult {
            success: output.status.success(),
            passed,
            failed,
            skipped,
            duration_ms,
            output: combined_output,
            tests,
            project_path: project_path.to_path_buf(),
        })
    }

    fn is_available(&self) -> bool {
        // Check if the test runner for this language is available
        let cmd = match self.language {
            TargetLanguage::Rust => "cargo",
            TargetLanguage::Python => "python",
            TargetLanguage::TypeScript | TargetLanguage::JavaScript => "npm",
            TargetLanguage::Go => "go",
            TargetLanguage::Java => "mvn",
            _ => return false,
        };

        Command::new(cmd)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn name(&self) -> &str {
        "ShellTestRunner"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_output_parsing() {
        let runner = ShellTestRunner::new(TargetLanguage::Rust);
        let output = r#"
running 3 tests
test tests::add ... ok
test tests::subtract ... ok
test tests::multiply ... FAILED

failures:

---- tests::multiply ----
assertion failed

test result: FAILED. 2 passed; 1 failed
"#;
        let (passed, failed, _skipped, tests) = runner.parse_rust_output(output);
        assert_eq!(passed, 2);
        assert_eq!(failed, 1);
        assert_eq!(tests.len(), 3);
    }

    #[test]
    fn test_pytest_output_parsing() {
        let runner = ShellTestRunner::new(TargetLanguage::Python);
        let output = r#"
test_add PASSED
test_subtract PASSED
test_multiply FAILED

======================== 2 passed, 1 failed ========================
"#;
        let (passed, failed, _, tests) = runner.parse_pytest_output(output);
        assert_eq!(passed, 2);
        assert_eq!(failed, 1);
        assert_eq!(tests.len(), 3);
    }
}
