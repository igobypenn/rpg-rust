//! Execution plan for Phase 3 code generation.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::checkpoint::CheckpointManager;
use crate::contract::ContractVerifier;
use crate::error::{GeneratorError, Result};
use crate::test_runner::{ShellTestRunner, TestConfig, TestRunner};
use crate::types::{
    ArchitectureDesign, ExecutionResult, TargetLanguage, TaskOutcome, TestError, TestResult,
};
use crate::TaskStatus;

#[cfg(feature = "opencode")]
use crate::agent::{Agent, AgentRegistry, CodeGenContext, Feedback, TestFailure};

pub struct ExecutionPlan {
    design: ArchitectureDesign,
    #[cfg(feature = "opencode")]
    agent: Box<dyn Agent>,
    #[cfg(not(feature = "opencode"))]
    client: crate::llm::OpenAIClient,
    test_runner: ShellTestRunner,
    #[allow(dead_code)]
    verifier: ContractVerifier,
    checkpoint: Option<Arc<RwLock<CheckpointManager>>>,
    max_iterations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CodeGenerationResponse {
    code: String,
    tests: String,
}

#[derive(Debug, Clone)]
struct TaskContext {
    task_id: String,
    file_path: PathBuf,
    component: String,
    interface: Option<crate::types::FileInterface>,
}

impl ExecutionPlan {
    /// Create with an agent (requires opencode feature).
    #[cfg(feature = "opencode")]
    pub fn with_agent(design: ArchitectureDesign, agent: Box<dyn Agent>) -> Self {
        let language = design.skeleton.language.clone();
        let test_runner =
            ShellTestRunner::new(TargetLanguage::parse(&language).unwrap_or(TargetLanguage::Rust));

        Self {
            design,
            agent,
            test_runner,
            verifier: ContractVerifier::new(),
            checkpoint: None,
            max_iterations: 5,
        }
    }

    /// Create with the default agent from registry (requires opencode feature).
    #[cfg(feature = "opencode")]
    pub fn new(design: ArchitectureDesign) -> Self {
        let mut registry = AgentRegistry::new();
        let agent = registry.take_default();
        Self::with_agent(design, agent)
    }

    /// Create with OpenAIClient (fallback without opencode feature).
    #[cfg(not(feature = "opencode"))]
    pub fn new_with_client(design: ArchitectureDesign, client: crate::llm::OpenAIClient) -> Self {
        let language = design.skeleton.language.clone();
        let test_runner = ShellTestRunner::new(
            TargetLanguage::from_str(&language).unwrap_or(TargetLanguage::Rust),
        );

        Self {
            design,
            client,
            test_runner,
            verifier: ContractVerifier::new(),
            checkpoint: None,
            max_iterations: 5,
        }
    }

    pub fn with_checkpoint(mut self, checkpoint: Arc<RwLock<CheckpointManager>>) -> Self {
        self.checkpoint = Some(checkpoint);
        self
    }

    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    pub async fn execute(&self) -> Result<ExecutionResult> {
        let mut result = ExecutionResult::new(self.design.plan_id);

        let tasks = self.collect_tasks();
        let total_tasks = tasks.len();

        for (idx, ctx) in tasks.iter().enumerate() {
            tracing::info!(
                "Executing task {}/{}: {}",
                idx + 1,
                total_tasks,
                ctx.task_id
            );

            let outcome = self.execute_task(ctx, &mut result).await;
            result.add_outcome(outcome);

            if let Some(ref checkpoint) = self.checkpoint {
                let mut mgr = checkpoint.write().await;
                let _ = mgr.set_execution_result(result.clone());
                mgr.save()?;
            }
        }

        result.complete();
        Ok(result)
    }

    fn collect_tasks(&self) -> Vec<TaskContext> {
        self.design
            .task_plan
            .all_tasks()
            .iter()
            .map(|task| {
                let interface = self.design.interfaces.get(&task.file_path).cloned();
                TaskContext {
                    task_id: task.task_id.clone(),
                    file_path: task.file_path.clone(),
                    component: task.subtree.clone(),
                    interface,
                }
            })
            .collect()
    }

    async fn execute_task(&self, ctx: &TaskContext, result: &mut ExecutionResult) -> TaskOutcome {
        let mut iterations = 0;
        let mut last_test_pass_rate = 0.0f32;
        let mut last_generated_code: Option<String> = None;

        #[cfg(feature = "opencode")]
        let mut feedback = Feedback::new();

        for iteration in 0..self.max_iterations {
            iterations = iteration + 1;

            #[cfg(feature = "opencode")]
            {
                feedback.iteration = iteration as u32;
                if let Some(ref code) = last_generated_code {
                    feedback.previous_code = Some(code.clone());
                }
            }

            #[cfg(feature = "opencode")]
            let generated = match self.generate_code_with_feedback(ctx, &feedback).await {
                Ok(code) => code,
                Err(e) => {
                    tracing::error!("Code generation failed for {}: {}", ctx.task_id, e);
                    return TaskOutcome {
                        task_id: ctx.task_id.clone(),
                        status: TaskStatus::Failed,
                        iterations,
                        test_pass_rate: 0.0,
                        generated_at: Utc::now(),
                    };
                }
            };

            #[cfg(not(feature = "opencode"))]
            let generated = match self.generate_code(ctx).await {
                Ok(code) => code,
                Err(e) => {
                    tracing::error!("Code generation failed for {}: {}", ctx.task_id, e);
                    return TaskOutcome {
                        task_id: ctx.task_id.clone(),
                        status: TaskStatus::Failed,
                        iterations,
                        test_pass_rate: 0.0,
                        generated_at: Utc::now(),
                    };
                }
            };

            last_generated_code = Some(generated.code.clone());
            result.add_file(ctx.file_path.clone(), generated.code.clone());
            result.add_file(
                ctx.file_path
                    .with_extension(format!("test.{}", self.test_file_extension())),
                generated.tests,
            );

            let test_result =
                self.run_tests(ctx.file_path.parent().unwrap_or(std::path::Path::new(".")));
            let test_pass_rate = if test_result.passed + test_result.failed > 0 {
                test_result.passed as f32 / (test_result.passed + test_result.failed) as f32
            } else {
                0.0
            };

            last_test_pass_rate = test_pass_rate;

            // Collect test errors for both result and feedback
            let test_errors: Vec<TestError> = test_result
                .tests
                .iter()
                .filter(|t| !t.passed)
                .map(|t| TestError {
                    test_name: t.name.clone(),
                    message: t.error.clone().unwrap_or_default(),
                    line: None,
                    stack_trace: t.stack_trace.clone(),
                })
                .collect();

            result.test_results.push(TestResult {
                task_id: ctx.task_id.clone(),
                passed: test_result.passed,
                failed: test_result.failed,
                errors: test_errors.clone(),
            });

            #[cfg(feature = "opencode")]
            {
                // Accumulate feedback for next iteration with test failures
                // Append new failures to existing ones so agent sees all failures across iterations
                let new_failures: Vec<TestFailure> = test_errors
                    .iter()
                    .map(|e| TestFailure {
                        test_name: e.test_name.clone(),
                        message: e.message.clone(),
                        line: e.line,
                    })
                    .collect();
                feedback.test_failures.extend(new_failures);
            }

            if test_result.success {
                tracing::info!(
                    "Task {} passed all tests after {} iterations",
                    ctx.task_id,
                    iterations
                );
                return TaskOutcome {
                    task_id: ctx.task_id.clone(),
                    status: TaskStatus::Completed,
                    iterations,
                    test_pass_rate: 1.0,
                    generated_at: Utc::now(),
                };
            }

            if test_pass_rate > 0.8 {
                tracing::warn!(
                    "Task {} has {:.0}% pass rate, attempting fix",
                    ctx.task_id,
                    test_pass_rate * 100.0
                );
            }
        }

        tracing::error!(
            "Task {} failed after {} iterations (last pass rate: {:.0}%)",
            ctx.task_id,
            iterations,
            last_test_pass_rate * 100.0
        );

        TaskOutcome {
            task_id: ctx.task_id.clone(),
            status: TaskStatus::Failed,
            iterations,
            test_pass_rate: last_test_pass_rate,
            generated_at: Utc::now(),
        }
    }

    /// Generate code using agent (opencode feature).
    #[cfg(feature = "opencode")]
    #[allow(dead_code)]
    async fn generate_code(&self, ctx: &TaskContext) -> Result<CodeGenerationResponse> {
        let interface = ctx
            .interface
            .clone()
            .unwrap_or_else(|| crate::types::FileInterface {
                path: ctx.file_path.clone(),
                units: vec![],
                imports: vec![],
            });

        let task =
            crate::ImplementationTask::new(&ctx.task_id, ctx.file_path.clone(), &ctx.component);

        let code_gen_ctx = CodeGenContext {
            task,
            interface,
            feedback: Feedback::new(),
        };

        let prompt = code_gen_ctx.compile(&self.agent.capabilities());
        let output = self.agent.execute(&prompt).await?;

        // Parse JSON response from agent output
        let json_str = output
            .as_json()
            .map(|v| v.to_string())
            .unwrap_or_else(|| output.to_text());

        let response: CodeGenerationResponse = serde_json::from_str(&json_str).map_err(|e| {
            GeneratorError::ParseFailed(format!("Failed to parse code response: {}", e))
        })?;

        Ok(response)
    }

    /// Generate code using agent with feedback from previous iterations (opencode feature).
    #[cfg(feature = "opencode")]
    async fn generate_code_with_feedback(
        &self,
        ctx: &TaskContext,
        feedback: &Feedback,
    ) -> Result<CodeGenerationResponse> {
        let interface = ctx
            .interface
            .clone()
            .unwrap_or_else(|| crate::types::FileInterface {
                path: ctx.file_path.clone(),
                units: vec![],
                imports: vec![],
            });

        let task =
            crate::ImplementationTask::new(&ctx.task_id, ctx.file_path.clone(), &ctx.component);

        let code_gen_ctx = CodeGenContext {
            task,
            interface,
            feedback: feedback.clone(),
        };

        let prompt = code_gen_ctx.compile(&self.agent.capabilities());
        let output = self.agent.execute(&prompt).await?;

        // Parse JSON response from agent output
        let json_str = output
            .as_json()
            .map(|v| v.to_string())
            .unwrap_or_else(|| output.to_text());

        let response: CodeGenerationResponse = serde_json::from_str(&json_str).map_err(|e| {
            GeneratorError::ParseFailed(format!("Failed to parse code response: {}", e))
        })?;

        Ok(response)
    }

    /// Generate code using OpenAIClient (fallback without opencode).
    #[cfg(not(feature = "opencode"))]
    async fn generate_code(&self, ctx: &TaskContext) -> Result<CodeGenerationResponse> {
        let interface_desc = if let Some(ref iface) = ctx.interface {
            format!(
                "Units: {}\nImports: {}",
                iface
                    .units
                    .iter()
                    .map(|u| format!("{}: {}", u.name, u.signature.as_deref().unwrap_or("?")))
                    .collect::<Vec<_>>()
                    .join(", "),
                iface.imports.join(", ")
            )
        } else {
            "No interface defined".to_string()
        };

        let prompt = format!(
            "Generate code for file: {}\nComponent: {}\n\n{}\n\nRespond with JSON: {{\"code\": \"...\", \"tests\": \"...\"}}",
            ctx.file_path.display(),
            ctx.component,
            interface_desc
        );

        let response: CodeGenerationResponse = self
            .client
            .complete_json_raw("", &prompt)
            .await
            .and_then(|v| self.client.deserialize(v))?;

        Ok(response)
    }

    fn run_tests(&self, project_path: &std::path::Path) -> crate::test_runner::TestRunResult {
        let config = TestConfig::new().with_timeout(60).with_parallel(false);

        self.test_runner
            .run_tests(project_path, &config)
            .unwrap_or_else(|_| crate::test_runner::TestRunResult {
                success: false,
                passed: 0,
                failed: 0,
                skipped: 0,
                duration_ms: 0,
                output: "Failed to run tests".to_string(),
                tests: vec![],
                project_path: project_path.to_path_buf(),
            })
    }

    fn test_file_extension(&self) -> &str {
        match self.design.skeleton.language.as_str() {
            "rust" => "rs",
            "python" => "py",
            "typescript" => "ts",
            "javascript" => "js",
            "go" => "go",
            "java" => "java",
            _ => "txt",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RepoSkeleton, TaskPlan};
    use uuid::Uuid;

    fn create_test_design() -> ArchitectureDesign {
        ArchitectureDesign::new(
            Uuid::new_v4(),
            RepoSkeleton::new(PathBuf::from("src"), "rust"),
            TaskPlan::new(),
        )
    }

    #[test]
    #[cfg(feature = "opencode")]
    fn test_execution_plan_creation_with_agent() {
        let design = create_test_design();
        let plan = ExecutionPlan::new(design);
        assert_eq!(plan.max_iterations, 5);
    }

    #[test]
    #[cfg(not(feature = "opencode"))]
    fn test_execution_plan_creation_with_client() {
        use crate::llm::LlmConfig;
        let design = create_test_design();
        let config = LlmConfig::new("test-key");
        let client = crate::llm::OpenAIClient::new(config).unwrap();

        let plan = ExecutionPlan::new_with_client(design, client);
        assert_eq!(plan.max_iterations, 5);
    }

    #[test]
    #[cfg(feature = "opencode")]
    fn test_execution_plan_with_max_iterations() {
        let design = create_test_design();
        let plan = ExecutionPlan::new(design).with_max_iterations(10);
        assert_eq!(plan.max_iterations, 10);
    }

    #[test]
    fn test_task_context_creation() {
        let ctx = TaskContext {
            task_id: "task_123".to_string(),
            file_path: PathBuf::from("src/main.rs"),
            component: "core".to_string(),
            interface: None,
        };

        assert_eq!(ctx.task_id, "task_123");
        assert_eq!(ctx.file_path, PathBuf::from("src/main.rs"));
    }
}
