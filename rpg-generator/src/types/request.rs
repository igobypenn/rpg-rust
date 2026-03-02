//! Generation request types.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A request to generate a codebase from a natural language description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    /// Natural language description of the project to generate.
    pub description: String,

    /// Target programming language.
    pub language: TargetLanguage,

    /// Optional constraints on the generation.
    #[serde(default)]
    pub constraints: Option<Constraints>,

    /// Unique identifier for this request.
    #[serde(default = "uuid::Uuid::new_v4")]
    pub id: uuid::Uuid,
}

impl GenerationRequest {
    /// Create a new generation request.
    pub fn new(description: impl Into<String>, language: TargetLanguage) -> Self {
        Self {
            description: description.into(),
            language,
            constraints: None,
            id: uuid::Uuid::new_v4(),
        }
    }

    /// Add constraints to the request.
    pub fn with_constraints(mut self, constraints: Constraints) -> Self {
        self.constraints = Some(constraints);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[derive(Default)]
pub enum TargetLanguage {
    #[default]
    Rust,
    Python,
    TypeScript,
    JavaScript,
    Go,
    Java,
    CSharp,
    Ruby,
}

impl TargetLanguage {
    /// Get the language name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Go => "go",
            Self::Java => "java",
            Self::CSharp => "csharp",
            Self::Ruby => "ruby",
        }
    }

    /// Get the default file extension for this language.
    pub fn default_extension(&self) -> &'static str {
        match self {
            Self::Rust => "rs",
            Self::Python => "py",
            Self::TypeScript => "ts",
            Self::JavaScript => "js",
            Self::Go => "go",
            Self::Java => "java",
            Self::CSharp => "cs",
            Self::Ruby => "rb",
        }
    }

    /// Get the test command for this language.
    pub fn test_command(&self) -> &'static str {
        match self {
            Self::Rust => "cargo test",
            Self::Python => "pytest",
            Self::TypeScript | Self::JavaScript => "npm test",
            Self::Go => "go test ./...",
            Self::Java => "mvn test",
            Self::CSharp => "dotnet test",
            Self::Ruby => "bundle exec rspec",
        }
    }

    /// Get the compile command for this language (if applicable).
    pub fn compile_command(&self) -> Option<&'static str> {
        match self {
            Self::Rust => Some("cargo build"),
            Self::Go => Some("go build ./..."),
            Self::Java => Some("mvn compile"),
            Self::TypeScript => Some("tsc --noEmit"),
            Self::CSharp => Some("dotnet build"),
            Self::Python | Self::JavaScript | Self::Ruby => None,
        }
    }

    /// Get the default test framework for this language.
    pub fn test_framework(&self) -> &'static str {
        match self {
            Self::Rust => "cargo test",
            Self::Python => "pytest",
            Self::TypeScript | Self::JavaScript => "jest",
            Self::Go => "go test",
            Self::Java => "junit",
            Self::CSharp => "xunit",
            Self::Ruby => "rspec",
        }
    }

    /// Parse from a string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "rust" => Some(Self::Rust),
            "python" | "py" => Some(Self::Python),
            "typescript" | "ts" => Some(Self::TypeScript),
            "javascript" | "js" => Some(Self::JavaScript),
            "go" | "golang" => Some(Self::Go),
            "java" => Some(Self::Java),
            "csharp" | "c#" => Some(Self::CSharp),
            "ruby" | "rb" => Some(Self::Ruby),
            _ => None,
        }
    }
}

impl std::fmt::Display for TargetLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}


/// Constraints on code generation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Constraints {
    /// Maximum number of files to generate.
    #[serde(default)]
    pub max_files: Option<usize>,

    /// Required dependencies (e.g., crate names, npm packages).
    #[serde(default)]
    pub required_dependencies: Vec<String>,

    /// Patterns to exclude from generation.
    #[serde(default)]
    pub excluded_patterns: Vec<String>,

    /// Style guide or conventions to follow.
    #[serde(default)]
    pub style_guide: Option<String>,

    /// Output directory for generated code.
    #[serde(default)]
    pub output_dir: Option<PathBuf>,
}

impl Constraints {
    /// Create empty constraints.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of files.
    pub fn with_max_files(mut self, max: usize) -> Self {
        self.max_files = Some(max);
        self
    }

    /// Add required dependencies.
    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.required_dependencies = deps;
        self
    }

    /// Set the output directory.
    pub fn with_output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.output_dir = Some(dir.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generation_request_creation() {
        let req = GenerationRequest::new("A REST API", TargetLanguage::Rust);
        assert_eq!(req.language, TargetLanguage::Rust);
        assert_eq!(req.description, "A REST API");
    }

    #[test]
    fn test_target_language_defaults() {
        assert_eq!(TargetLanguage::Rust.default_extension(), "rs");
        assert_eq!(TargetLanguage::Python.default_extension(), "py");
    }

    #[test]
    fn test_constraints_builder() {
        let constraints = Constraints::new()
            .with_max_files(50)
            .with_dependencies(vec!["serde".to_string()])
            .with_output_dir("./output");

        assert_eq!(constraints.max_files, Some(50));
        assert_eq!(constraints.required_dependencies.len(), 1);
    }
}
