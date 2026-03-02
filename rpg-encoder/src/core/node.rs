//! Node types for the Repository Planning Graph.
//!
//! This module defines the core node and edge types that form the RPG graph.

//!
//! ## Categories
//!
//! Nodes are organized into categories that determine their role in the graph:
//!
//! | Category | Description |
//! |----------|-------------|
//! | `Repository` | Root node for the entire repository |
//! | `Directory` | Directory within repository |
//! | `File` | Source file |
//! | `Module` | Module/namespace |
//! | `Type` | Type definition (struct, enum, class, interface) |
//! | `Function` | Function/method definition |
//! | `Variable` | Variable binding |
//! | `Import` | Import/use statement |
//! | `Constant` | Constant definition |
//! | `Field` | Field/property on a type |
//! | `Parameter` | Function parameter |
//! | `Feature` | Feature flag or configuration |
//! | `Component` | Logical component grouping |

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::id::NodeId;
use super::location::SourceLocation;

/// Classification of code entities in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeCategory {
    /// Repository root node
    Repository,
    /// Directory within repository
    Directory,
    /// Source file
    File,
    /// Module/namespace
    Module,
    /// Type definition (struct, enum, class, interface)
    Type,
    /// Function/method definition
    Function,
    /// Variable binding
    Variable,
    /// Import/use statement
    Import,
    /// Constant definition
    Constant,
    /// Field/property on a type
    Field,
    /// Function parameter
    Parameter,
    /// Feature flag or configuration
    Feature,
    /// Logical component grouping
    Component,
    /// High-level functional centroid (V^H node)
    FunctionalCentroid,
}

/// Hierarchy level for nodes in the RPG.
/// 
/// Per the paper, nodes are classified as either:
/// - V^L (Low): Implementation-level nodes (functions, types, etc.)
/// - V^H (High): Functional centroid nodes (abstract feature areas)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeLevel {
    /// Low-level implementation node (V^L)
    #[default]
    Low,
    /// Intermediate-level category/subcategory node
    Intermediate,
    /// High-level functional centroid (V^H)
    High,
}

/// Source location reference (line-based).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRef {
    /// Starting line number (1-based).
    pub start_line: usize,
    /// Ending line number (inclusive).
    pub end_line: usize,
}

/// Represents a code entity in the repository graph.
///
/// Nodes are the primary building blocks of the RPG graph, representing
/// various code-level constructs such as functions, types, modules, and imports.
///
/// # Example
///
/// ```
/// use rpg_encoder::{Node, NodeCategory, NodeId};
///
/// let node = Node::new(
///     NodeId::new(0),
///     NodeCategory::Function,
///     "function",
///     "rust",
///     "process_data"
/// )
/// .with_documentation("Processes input data")
/// .with_signature("fn process_data(input: &str) -> Result<Vec<String>, Error>");
///
/// assert_eq!(node.name, "process_data");
/// assert_eq!(node.category, NodeCategory::Function);
/// assert!(node.documentation.is_some());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier for this node.
    pub id: NodeId,
    /// Category determining the node's role in the graph.
    pub category: NodeCategory,
    /// Kind of code construct (e.g., "fn", "struct", "enum").
    pub kind: String,
    /// Programming language (e.g., "rust", "python").
    pub language: String,
    /// Name of the entity (function name, type name, etc.).
    pub name: String,
    /// File path where this entity is defined.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// Source location (file, line, column).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<SourceLocation>,
    /// Additional metadata (custom key-value pairs).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Brief description of the entity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Feature flags associated with this entity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    /// Feature path (e.g., "auth/login" for a login feature).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_path: Option<String>,
    /// Function/method signature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Source code reference (line range).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Semantic feature description (lifted from code for search/analysis).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_feature: Option<String>,
    /// Hierarchy level (V^L for implementation, V^H for functional centroid).
    #[serde(default)]
    pub node_level: NodeLevel,
    /// Documentation comment (doc comment).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
}

impl Node {
    /// Create a new node.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this node
    /// * `category` - Category determining the node's role
    /// * `kind` - Kind of code construct (e.g., "fn", "struct")
    /// * `language` - Programming language (e.g., "rust", "python")
    /// * `name` - Name of the entity
    #[must_use = "Node must be used"]
    pub fn new(
        id: NodeId,
        category: NodeCategory,
        kind: impl Into<String>,
        language: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            id,
            category,
            kind: kind.into(),
            language: language.into(),
            name: name.into(),
            path: None,
            location: None,
            metadata: HashMap::new(),
            description: None,
            features: Vec::new(),
            feature_path: None,
            signature: None,
            documentation: None,
            source_ref: None,
            semantic_feature: None,
            node_level: NodeLevel::default(),
        }
    }

    /// Set the file path.
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Set source location.
    pub fn with_location(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }

    /// Add metadata entry.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Set description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set feature flags.
    pub fn with_features(mut self, features: Vec<String>) -> Self {
        self.features = features;
        self
    }

    /// Set feature path.
    pub fn with_feature_path(mut self, path: impl Into<String>) -> Self {
        self.feature_path = Some(path.into());
        self
    }

    /// Set function/method signature.
    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }

    /// Set documentation comment.
    pub fn with_documentation(mut self, documentation: impl Into<String>) -> Self {
        self.documentation = Some(documentation.into());
        self
    }

    /// Set semantic feature (lifted description for search/analysis).
    pub fn with_semantic_feature(mut self, feature: impl Into<String>) -> Self {
        self.semantic_feature = Some(feature.into());
        self
    }

    /// Set node level (V^L or V^H).
    pub fn with_node_level(mut self, level: NodeLevel) -> Self {
        self.node_level = level;
        self
    }
}

impl std::fmt::Display for NodeCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeCategory::Repository => write!(f, "repository"),
            NodeCategory::Directory => write!(f, "directory"),
            NodeCategory::File => write!(f, "file"),
            NodeCategory::Module => write!(f, "module"),
            NodeCategory::Type => write!(f, "type"),
            NodeCategory::Function => write!(f, "function"),
            NodeCategory::Variable => write!(f, "variable"),
            NodeCategory::Import => write!(f, "import"),
            NodeCategory::Constant => write!(f, "constant"),
            NodeCategory::Field => write!(f, "field"),
            NodeCategory::Parameter => write!(f, "parameter"),
            NodeCategory::Feature => write!(f, "feature"),
            NodeCategory::Component => write!(f, "component"),
            NodeCategory::FunctionalCentroid => write!(f, "functional_centroid"),
        }
    }
}
