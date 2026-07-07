//! # rpg-encoder
//!
//! Repository Planning Graph encoder for multi-language code analysis.
//!
//! ## Overview
//!
//! rpg-encoder parses source code repositories into a directed graph structure
//! where nodes represent code entities (functions, types, modules, etc.) and edges
//! represent relationships (call graphs, importsgraph, containment, inheritance).
//!
//! ## Features
//!
//! ### Core Features (always available)
//!
//! - **Multi-language support**: Rust, Python, Go, C/C++, Java, JavaScript/TypeScript, Ruby, Swift, Lua, Haskell, C#, Scala (always compiled)
//! - **Incremental processing**: Snapshot-based diff system for fast re-parsing
//! - **FFI detection**: Automatic foreign function interface discovery
//!
//! ### Feature Flags
//!
//! All tree-sitter language parsers are unconditional dependencies — no feature flags needed.
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `llm` | LLM-powered semantic feature extraction |
//! | `embeddings` | Vector embeddings (FlatIndex + optional zvec) |
//! | `zvec` | zvec vector DB backend (native C++ dependency) |
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use rpg_encoder::{RpgEncoder, RpgGraph};
//! use std::path::Path;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut encoder = RpgEncoder::new()?;
//!     let result = encoder.encode(Path::new("./src"))?;
//!
//!     println!("Files: {}, Nodes: {}, Edges: {}",
//!         result.files_processed,
//!         result.graph.node_count(),
//!         result.graph.edge_count());
//!
//!     // Export to JSON
//!     println!("{}", encoder.to_json_compact()?);
//!
//!     // Export to JSON (pretty-printed)
//!     println!("{}", encoder.to_json()?);
//!
//!     // Export graph for further processing
//!     let json = encoder.to_json_compact()?;
//!     let graph: RpgGraph = serde_json::from_str(&json)?;
//!     // Process the graph...
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Incremental Processing
//!
//! Use snapshots to track changes between codebase versions:
//!
//! ```rust,ignore
//! use rpg_encoder::{RpgEncoder, RpgSnapshot};
//! use std::path::Path;
//!
//! let mut encoder = RpgEncoder::new().unwrap();
//!
//! // Initial snapshot
//! let snapshot = RpgSnapshot::from_dir(Path::new("./src")).unwrap();
//!
//! // ... make some changes
//! let new_snapshot = RpgSnapshot::from_dir(Path::new("./src")).unwrap();
//!
//! let stats = snapshot.stats();
//! println!("Units: {} (added, {} changed, {} deleted)",
//!     stats.units_added, stats.units_changed, stats.units_deleted
//! );
//! ```
//!
//! ## Custom Parsers
//!
//! Register custom language parsers by implementing [`LanguageParser`] trait
//! and using [`RpgEncoder::with_parser`]:
//!
//! ```rust,ignore
//! use rpg_encoder::{LanguageParser, RpgEncoder};
//! use std::path::Path;
//!
//! struct MyLangParser;
//!
//! impl LanguageParser for MyLangParser {
//!     fn language_name(&self) -> &str { "mylang" }
//!     fn file_extensions(&self) -> &[&str] { &["myl"] }
//! }
//!
//! let encoder = RpgEncoder::new().unwrap()
//!     .with_parser(Box::new(MyLangParser));
//!
//! let result = encoder.encode(Path::new("./my-project")).unwrap();
//! ```
//!
//! ## Graph API
//!
//! ```rust
//! use rpg_encoder::{GraphBuilder, NodeCategory};
//! use std::path::Path;
//!
//! let graph = GraphBuilder::new()
//!     .with_repo("my-repo", Path::new("."))
//!     .add_file(Path::new("src/main.rs"), "rust")
//!     .build();
//!
//! // Query the graph
//! for node in graph.nodes() {
//!     println!("{:?}: {}", node.category, node.name);
//! }
//! ```
//!
//! ## Error Handling
//!
//! ```rust,ignore
//! use rpg_encoder::error::{RpgError, ResultExt};
//!
//! fn load_config() -> rpg_encoder::error::Result<()> {
//!     let config = std::fs::read_to_string("config.yaml")
//!         .map_err(|e| RpgError::Config(e.to_string()))?;
//!     Ok(())
//! }
//! ```
//!
//! ## Encoder Types
//!
//! [`RpgEncoder`] - Main entry point for encoding repositories
//!
//! [`GraphBuilder`] - Builder for constructing graphs manually
//!
//! [`FileWalker`] - Directory walker
//!
//! [`ParseFailure`] - Information about parse failures details
//!
//! [`SerializedGraph`] - Serialized graph format
//!
//! [`RpgGraph`] - The main graph type
//!
//! [`Node`] - Represents a code entity in the graph.
//!
//! [`Edge`] - Represents a relationship between two entities.
//!
//! [`EdgeType`] - Types of relationships
//!
//! [`NodeCategory`] - Categories of code entities
//!
//! [`NodeId`] - Unique identifier for each node
//!
//! [`SourceLocation`] - Location in source code
//!
//! [`SourceRef`] - Source code reference
//!
//! [`RpgSnapshot`] - Snapshot of codebase state
//!
//! [`RpgEvolution`] - Evolution tracking
//!
//! [`DiffStats`] - Diff statistics
//!
//! [`ParseErrorCategory`] - Category of parse error
//!
//! [`ParseFailure`] - Detailed parse failure info
//!
//! [`LanguageParser`] - Trait for language parsers
//!
//! [`ParserRegistry`] - Registry for language parsers

pub mod core;
pub mod encoder;
pub mod error;
pub mod export;
pub mod incremental;
pub mod languages;
pub mod parser;
pub mod scip;
pub mod storage;

#[cfg(feature = "llm")]
pub mod agents;
#[cfg(feature = "llm")]
pub mod llm;
#[cfg(feature = "embeddings")]
pub mod embeddings;

#[cfg(feature = "llm")]
pub use agents::{ExtractionScope, FeatureExtractor, SemanticConfig};
#[cfg(feature = "llm")]
pub use llm::{LlmConfig, OpenAIClient};
#[cfg(feature = "embeddings")]
pub use embeddings::{
    EmbeddingClient, EmbeddingConfig, EmbeddingError, EmbeddingIndex, EmbeddingStore, Embedder,
    FlatIndex,
};
#[cfg(feature = "zvec")]
pub use embeddings::{ZvecIndex, ZvecIndexKind};

pub use core::{
    Edge, EdgeType, Node, NodeCategory, NodeId, NodeLevel, RpgGraph, SourceLocation,
};
pub use encoder::{
    to_json, to_json_compact, EncodeResult, FileWalker, FunctionalAbstraction,
    FunctionalCentroid, GraphBuilder, RpgEncoder, SerializedGraph,
};
pub use error::{Result, RpgError};
pub use incremental::{
    compute_hash, generate_diff, CachedUnit, CodeUnit, DiffStats, EvolutionSummary, FileDiff,
    RpgEvolution, RpgSnapshot, UnitType, SNAPSHOT_VERSION,
};
pub use parser::{LanguageParser, ParserRegistry};
pub use storage::{BaseSnapshot, RpgStore, RPG_DIR};
pub use export::{to_cypher, to_dot, to_graphml};
pub use scip::{enrich_graph as enrich_graph_scip, ScipIndex};

pub mod utils;
