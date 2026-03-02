//! Error types for rpg-encoder.
//!
//! All errors are consolidated into [`RpgError`] with the [`Result<T>`] type alias
//! for ergonomic error handling.
//!
//! # Error Categories
//!
//! | Category | Variants | Recoverable |
//! |----------|----------|-------------|
//! | I/O | `Io`, `WalkError` | No |
//! | Parsing | `ParseError`, `TreeSitterError` | Yes |
//! | Configuration | `Config`, `MissingEnv` | No |
//! | Runtime | `LockAcquisition`, `HttpClient` | Varies |
//!
//! # Example
//!
//! ```rust,ignore
//! use rpg_encoder::error::{RpgError, Result, ResultExt};
//!
//! fn process_file() -> Result<()> {
//!     // Convert io::Error to RpgError
//!     let content = std::fs::read_to_string("missing.txt")
//!         .map_err(RpgError::Io)
//!         .context("loading config file")?;
//!     Ok(())
//! }
//! ```

use std::path::{Path, PathBuf};
use thiserror::Error;

/// Result type for rpg-encoder operations.
pub type Result<T> = std::result::Result<T, RpgError>;

/// Errors that can occur during encoding operations.
#[derive(Debug, Error)]
pub enum RpgError {
    /// I/O error reading files or directories.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Parse error with full source context.
    #[error("Parse error in {file}:{line}:{column}: {message}")]
    ParseError {
        /// File containing the error
        file: PathBuf,
        /// Line number (1-based)
        line: usize,
        /// Column number (1-based)
        column: usize,
        /// Error message
        message: String,
    },

    /// Tree-sitter parsing failure.
    #[error("Tree-sitter error in {file}: {message}")]
    TreeSitterError {
        /// File containing the error
        file: PathBuf,
        /// Error message
        message: String,
    },

    /// Requested language is not supported.
    #[error("Language not supported: {0}")]
    LanguageNotSupported(String),

    /// No parser registered for the file extension.
    #[error("No parser found for file: {0}")]
    NoParser(String),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Invalid or non-existent path.
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// Requested node does not exist in graph.
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    /// Directory walk error (from ignore crate).
    #[error("Walk error: {0}")]
    WalkError(#[from] ignore::Error),

    /// Parser initialization failed.
    #[error("Parser initialization failed for {language}: {message}")]
    ParserInit {
        /// Language that failed to initialize
        language: String,
        /// Error message
        message: String,
    },

    /// Graph not available - encode() not called.
    #[error("Graph not available - call encode() first")]
    NotEncoded,

    /// Failed to acquire mutex/lock.
    #[error("Failed to acquire lock: {0}")]
    LockAcquisition(String),

    /// HTTP client error (embedding API, LLM, etc.).
    #[error("HTTP client error: {0}")]
    HttpClient(String),

    /// Required environment variable not set.
    #[error("Required environment variable not set: {var}")]
    MissingEnv {
        /// Variable name
        var: String,
    },

    /// Empty response from external service.
    #[error("Empty response: {context}")]
    EmptyResponse {
        /// Context describing the operation
        context: String,
    },

    /// Failed to parse response from external service.
    #[error("Failed to parse response: {context}")]
    ResponseParse {
        /// Context describing what was being parsed
        context: String,
    },

    /// Path manipulation error.
    #[error("Path error during {operation}: {path}")]
    PathError {
        /// Path that caused the error
        path: String,
        /// Operation being performed
        operation: String,
    },

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Incremental processing error.
    #[error("Incremental processing error: {0}")]
    Incremental(String),

    #[cfg(feature = "llm")]
    #[error("LLM error: {0}")]
    Llm(#[from] crate::llm::LlmError),
}

impl RpgError {
    /// Create a parse error with file and location context.
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

    /// Create a tree-sitter error with file context.
    pub fn tree_sitter_error(file: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::TreeSitterError {
            file: file.into(),
            message: message.into(),
        }
    }

    /// Create a parser initialization error.
    pub fn parser_init(language: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ParserInit {
            language: language.into(),
            message: message.into(),
        }
    }

    /// Check if this error is recoverable (processing can continue).
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Self::ParseError { .. } | Self::TreeSitterError { .. })
    }

    /// Check if this is an I/O error.
    pub fn is_io_error(&self) -> bool {
        matches!(self, Self::Io(_) | Self::WalkError(_))
    }

    /// Check if this is a parsing error.
    pub fn is_parse_error(&self) -> bool {
        matches!(self, Self::ParseError { .. } | Self::TreeSitterError { .. })
    }

    /// Add context to the error message.
    pub fn context(self, ctx: impl Into<String>) -> Self {
        let ctx = ctx.into();
        match self {
            Self::ParseError {
                file,
                line,
                column,
                message,
            } => Self::ParseError {
                file,
                line,
                column,
                message: format!("{}: {}", ctx, message),
            },
            Self::TreeSitterError { file, message } => Self::TreeSitterError {
                file,
                message: format!("{}: {}", ctx, message),
            },
            Self::ParserInit { language, message } => Self::ParserInit {
                language,
                message: format!("{}: {}", ctx, message),
            },
            Self::NoParser(path) => Self::NoParser(format!("{}: {}", ctx, path)),
            Self::InvalidPath(path) => Self::InvalidPath(format!("{}: {}", ctx, path)),
            Self::NodeNotFound(id) => Self::NodeNotFound(format!("{}: {}", ctx, id)),
            Self::LockAcquisition(msg) => Self::LockAcquisition(format!("{}: {}", ctx, msg)),
            Self::HttpClient(msg) => Self::HttpClient(format!("{}: {}", ctx, msg)),
            Self::EmptyResponse { context } => Self::EmptyResponse {
                context: format!("{}: {}", ctx, context),
            },
            Self::ResponseParse { context } => Self::ResponseParse {
                context: format!("{}: {}", ctx, context),
            },
            Self::PathError { path, operation } => Self::PathError {
                path,
                operation: format!("{}: {}", ctx, operation),
            },
            Self::Config(msg) => Self::Config(format!("{}: {}", ctx, msg)),
            Self::Incremental(msg) => Self::Incremental(format!("{}: {}", ctx, msg)),
            Self::MissingEnv { var } => Self::MissingEnv {
                var: format!("{}: {}", ctx, var),
            },
            Self::LanguageNotSupported(lang) => {
                Self::LanguageNotSupported(format!("{}: {}", ctx, lang))
            }
            Self::NotEncoded => Self::Config(ctx),
            Self::Io(e) => Self::Io(std::io::Error::new(e.kind(), format!("{}: {}", ctx, e))),
            Self::JsonError(e) => Self::ResponseParse {
                context: format!("{}: {}", ctx, e),
            },
            Self::WalkError(e) => Self::WalkError(ignore::Error::from(std::io::Error::other(
                format!("{}: {}", ctx, e),
            ))),
            #[cfg(feature = "llm")]
            Self::Llm(e) => Self::HttpClient(format!("{}: {}", ctx, e)),
        }
    }
}

/// Extension trait for adding context to Results.
pub trait ResultExt<T> {
    /// Add context to the error.
    fn context(self, ctx: impl Into<String>) -> Result<T>;

    /// Add context lazily, only computed if error.
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T> ResultExt<T> for Result<T> {
    fn context(self, ctx: impl Into<String>) -> Result<T> {
        self.map_err(|e| e.context(ctx))
    }

    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| e.context(f()))
    }
}

/// Category of parse error for filtering and reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorCategory {
    /// Syntax error in source code.
    Syntax,
    /// File encoding issue (UTF-8, etc.).
    Encoding,
    /// File exceeds size limits.
    SizeLimit,
    /// Parser internal error.
    Internal,
    /// I/O error reading file.
    Io,
    /// Tree-sitter parsing failure.
    TreeSitter,
}

/// Detailed information about a parse failure.
#[derive(Debug, Clone)]
pub struct ParseFailure {
    /// Path to the file that failed.
    pub path: PathBuf,
    /// Human-readable error message.
    pub error: String,
    /// Line number if available (1-based).
    pub line: Option<usize>,
    /// Column number if available (1-based).
    pub column: Option<usize>,
    /// Error category for filtering.
    pub category: ParseErrorCategory,
}

impl ParseFailure {
    /// Create a ParseFailure from an RpgError.
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

    /// Check if this error has location information.
    pub fn has_location(&self) -> bool {
        self.line.is_some() && self.column.is_some()
    }

    /// Format as a compiler-style error message.
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
    fn test_is_recoverable() {
        let parse_err = RpgError::parse_error(Path::new("test.rs"), 1, 1, "error");
        assert!(parse_err.is_recoverable());

        let io_err = RpgError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        ));
        assert!(!io_err.is_recoverable());
    }

    #[test]
    fn test_is_io_error() {
        let io_err = RpgError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        ));
        assert!(io_err.is_io_error());

        let parse_err = RpgError::parse_error(Path::new("test.rs"), 1, 1, "error");
        assert!(!parse_err.is_io_error());
    }

    #[test]
    fn test_is_parse_error() {
        let parse_err = RpgError::parse_error(Path::new("test.rs"), 1, 1, "error");
        assert!(parse_err.is_parse_error());

        let ts_err = RpgError::tree_sitter_error(Path::new("test.rs"), "failed");
        assert!(ts_err.is_parse_error());

        let io_err = RpgError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        ));
        assert!(!io_err.is_parse_error());
    }

    #[test]
    fn test_context_parse_error() {
        let err = RpgError::parse_error(Path::new("test.rs"), 10, 5, "unexpected token");
        let with_context = err.context("while processing module");

        match with_context {
            RpgError::ParseError { message, .. } => {
                assert!(message.contains("while processing module"));
                assert!(message.contains("unexpected token"));
            }
            _ => panic!("Expected ParseError"),
        }
    }

    #[test]
    fn test_context_io_error() {
        let err = RpgError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        let with_context = err.context("reading source file");

        match with_context {
            RpgError::Io(e) => {
                assert!(e.to_string().contains("reading source file"));
            }
            _ => panic!("Expected Io"),
        }
    }

    #[test]
    fn test_result_ext_context() {
        let result: Result<()> = Err(RpgError::InvalidPath("bad/path".to_string()));
        let with_context = result.context("loading config");

        match with_context {
            Err(RpgError::InvalidPath(msg)) => {
                assert!(msg.contains("loading config"));
            }
            _ => panic!("Expected InvalidPath"),
        }
    }

    #[test]
    fn test_result_ext_with_context() {
        let result: Result<()> = Err(RpgError::InvalidPath("bad/path".to_string()));
        let with_context = result.with_context(|| format!("processing {}", "config"));

        match with_context {
            Err(RpgError::InvalidPath(msg)) => {
                assert!(msg.contains("processing config"));
            }
            _ => panic!("Expected InvalidPath"),
        }
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
    fn test_language_not_supported() {
        let err = RpgError::LanguageNotSupported("brainfuck".to_string());
        assert!(err.to_string().contains("Language not supported"));
        assert!(err.to_string().contains("brainfuck"));
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
    fn test_missing_env() {
        let err = RpgError::MissingEnv {
            var: "API_KEY".to_string(),
        };
        assert!(err
            .to_string()
            .contains("Required environment variable not set"));
        assert!(err.to_string().contains("API_KEY"));
    }

    #[test]
    fn test_empty_response() {
        let err = RpgError::EmptyResponse {
            context: "embedding API".to_string(),
        };
        assert!(err.to_string().contains("Empty response"));
        assert!(err.to_string().contains("embedding API"));
    }

    #[test]
    fn test_response_parse() {
        let err = RpgError::ResponseParse {
            context: "invalid JSON".to_string(),
        };
        assert!(err.to_string().contains("Failed to parse response"));
        assert!(err.to_string().contains("invalid JSON"));
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
