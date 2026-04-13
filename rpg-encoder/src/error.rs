//! Error types for rpg-encoder.

use std::path::{Path, PathBuf};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, RpgError>;

#[derive(Debug, Error)]
pub enum RpgError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error in {file}:{line}:{column}: {message}")]
    ParseError {
        file: PathBuf,
        line: usize,
        column: usize,
        message: String,
    },

    #[error("Tree-sitter error in {file}: {message}")]
    TreeSitterError { file: PathBuf, message: String },

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Parser initialization failed for {language}: {message}")]
    ParserInit { language: String, message: String },

    #[error("Graph not available - call encode() first")]
    NotEncoded,

    #[error("Failed to acquire lock: {0}")]
    LockAcquisition(String),

    #[error("HTTP client error: {0}")]
    HttpClient(String),

    #[error("No parser found for file: {0}")]
    NoParser(String),

    #[error("Empty response: {context}")]
    EmptyResponse { context: String },

    #[error("Failed to parse response: {context}")]
    ResponseParse { context: String },

    #[error("Path error during {operation}: {path}")]
    PathError { path: String, operation: String },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Incremental processing error: {0}")]
    Incremental(String),

    #[cfg(feature = "llm")]
    #[error("LLM error: {0}")]
    Llm(#[from] crate::llm::LlmError),
}

impl RpgError {
    pub fn parse_error(
        file: impl Into<PathBuf>,
        line: usize,
        column: usize,
        message: impl Into<String>,
    ) -> Self {
        Self::ParseError {
            file: file.into(),
            line,
            column,
            message: message.into(),
        }
    }

    pub fn tree_sitter_error(file: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::TreeSitterError {
            file: file.into(),
            message: message.into(),
        }
    }

    pub fn parser_init(language: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ParserInit {
            language: language.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorCategory {
    Syntax,
    Internal,
    Io,
    TreeSitter,
}

#[derive(Debug, Clone)]
pub struct ParseFailure {
    pub path: PathBuf,
    pub error: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub category: ParseErrorCategory,
}

impl ParseFailure {
    pub fn from_error(path: &Path, error: &RpgError) -> Self {
        match error {
            RpgError::ParseError {
                line,
                column,
                message,
                ..
            } => Self {
                path: path.to_path_buf(),
                error: message.clone(),
                line: Some(*line),
                column: Some(*column),
                category: ParseErrorCategory::Syntax,
            },
            RpgError::TreeSitterError { message, .. } => Self {
                path: path.to_path_buf(),
                error: message.clone(),
                line: None,
                column: None,
                category: ParseErrorCategory::TreeSitter,
            },
            RpgError::Io(e) => Self {
                path: path.to_path_buf(),
                error: e.to_string(),
                line: None,
                column: None,
                category: ParseErrorCategory::Io,
            },
            _ => Self {
                path: path.to_path_buf(),
                error: error.to_string(),
                line: None,
                column: None,
                category: ParseErrorCategory::Internal,
            },
        }
    }

    pub fn has_location(&self) -> bool {
        self.line.is_some() && self.column.is_some()
    }

    pub fn to_diagnostic(&self) -> String {
        if let (Some(line), Some(col)) = (self.line, self.column) {
            format!(
                "{}:{}:{}: error: {}",
                self.path.display(),
                line,
                col,
                self.error
            )
        } else {
            format!("{}: error: {}", self.path.display(), self.error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: RpgError = io_err.into();
        assert!(matches!(err, RpgError::Io(_)));
        assert!(err.to_string().contains("IO error"));
    }

    #[test]
    fn test_parse_error_creation() {
        let err = RpgError::parse_error(Path::new("test.rs"), 10, 5, "unexpected token");
        match err {
            RpgError::ParseError {
                file,
                line,
                column,
                message,
            } => {
                assert_eq!(file, PathBuf::from("test.rs"));
                assert_eq!(line, 10);
                assert_eq!(column, 5);
                assert_eq!(message, "unexpected token");
            }
            _ => panic!("Expected ParseError"),
        }
    }

    #[test]
    fn test_parse_error_message() {
        let err = RpgError::parse_error(Path::new("src/main.rs"), 42, 10, "expected semicolon");
        assert!(err.to_string().contains("src/main.rs"));
        assert!(err.to_string().contains("42:10"));
        assert!(err.to_string().contains("expected semicolon"));
    }

    #[test]
    fn test_tree_sitter_error() {
        let err = RpgError::tree_sitter_error(Path::new("test.rs"), "parse failed");
        assert!(err.to_string().contains("Tree-sitter error"));
        assert!(err.to_string().contains("test.rs"));
    }

    #[test]
    fn test_parser_init_error() {
        let err = RpgError::parser_init("python", "tree-sitter load failed");
        assert!(err.to_string().contains("Parser initialization failed"));
        assert!(err.to_string().contains("python"));
    }

    #[test]
    fn test_parse_failure_from_parse_error() {
        let path = PathBuf::from("src/lib.rs");
        let err = RpgError::parse_error(&path, 42, 10, "missing semicolon");
        let failure = ParseFailure::from_error(&path, &err);

        assert_eq!(failure.path, path);
        assert_eq!(failure.line, Some(42));
        assert_eq!(failure.column, Some(10));
        assert_eq!(failure.category, ParseErrorCategory::Syntax);
        assert!(failure.has_location());
    }

    #[test]
    fn test_parse_failure_from_io_error() {
        let path = PathBuf::from("src/missing.rs");
        let err = RpgError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        ));
        let failure = ParseFailure::from_error(&path, &err);

        assert_eq!(failure.path, path);
        assert_eq!(failure.line, None);
        assert_eq!(failure.category, ParseErrorCategory::Io);
        assert!(!failure.has_location());
    }

    #[test]
    fn test_parse_failure_diagnostic() {
        let path = PathBuf::from("src/lib.rs");
        let err = RpgError::parse_error(&path, 42, 10, "missing semicolon");
        let failure = ParseFailure::from_error(&path, &err);

        let diagnostic = failure.to_diagnostic();
        assert!(diagnostic.contains("src/lib.rs:42:10"));
        assert!(diagnostic.contains("error:"));
        assert!(diagnostic.contains("missing semicolon"));
    }

    #[test]
    fn test_parse_failure_diagnostic_no_location() {
        let path = PathBuf::from("src/lib.rs");
        let err = RpgError::tree_sitter_error(&path, "parse failed");
        let failure = ParseFailure::from_error(&path, &err);

        let diagnostic = failure.to_diagnostic();
        assert!(diagnostic.contains("src/lib.rs"));
        assert!(diagnostic.contains("error:"));
        assert!(!diagnostic.contains(":42:"));
    }

    #[test]
    fn test_no_parser() {
        let err = RpgError::NoParser("unknown.ext".to_string());
        assert!(err.to_string().contains("No parser found"));
        assert!(err.to_string().contains("unknown.ext"));
    }

    #[test]
    fn test_json_error() {
        let json_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let err: RpgError = json_err.into();
        assert!(matches!(err, RpgError::JsonError(_)));
        assert!(err.to_string().contains("JSON"));
    }

    #[test]
    fn test_node_not_found() {
        let err = RpgError::NodeNotFound("node_123".to_string());
        assert!(err.to_string().contains("Node not found"));
    }

    #[test]
    fn test_not_encoded() {
        let err = RpgError::NotEncoded;
        assert!(err.to_string().contains("Graph not available"));
    }

    #[test]
    fn test_lock_acquisition() {
        let err = RpgError::LockAcquisition("parser mutex".to_string());
        assert!(err.to_string().contains("Failed to acquire lock"));
        assert!(err.to_string().contains("parser mutex"));
    }

    #[test]
    fn test_http_client() {
        let err = RpgError::HttpClient("connection timeout".to_string());
        assert!(err.to_string().contains("HTTP client error"));
        assert!(err.to_string().contains("connection timeout"));
    }

    #[test]
    fn test_path_error() {
        let err = RpgError::PathError {
            path: "/some/path".to_string(),
            operation: "strip prefix".to_string(),
        };
        assert!(err.to_string().contains("Path error"));
        assert!(err.to_string().contains("/some/path"));
        assert!(err.to_string().contains("strip prefix"));
    }

    #[test]
    fn test_config_error() {
        let err = RpgError::Config("invalid yaml".to_string());
        assert!(err.to_string().contains("Configuration error"));
        assert!(err.to_string().contains("invalid yaml"));
    }

    #[test]
    fn test_incremental_error() {
        let err = RpgError::Incremental("snapshot corrupted".to_string());
        assert!(err.to_string().contains("Incremental processing error"));
        assert!(err.to_string().contains("snapshot corrupted"));
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RpgError>();
    }

    #[test]
    fn test_result_type() {
        fn returns_result() -> Result<String> {
            Ok("success".to_string())
        }
        assert!(returns_result().is_ok());
    }
}
