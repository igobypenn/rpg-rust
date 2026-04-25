//! Repository encoder module
//!
//! This module provides the main encoding pipeline for converting source code
//! repositories into RPG graphs.

mod builder;
mod functional;
mod output;
mod validation;
mod walker;

pub use builder::GraphBuilder;
pub use output::{serialize_graph, to_json, to_json_compact, SerializedEdge, SerializedGraph, SerializedNode};
pub use validation::ValidationReport;
pub use walker::FileWalker;

// Functional abstraction types for forward path (generator)
pub use functional::{
    AbstractionResult, CollectedFeature, FunctionalAbstraction, FunctionalCentroid,
};

use std::path::{Path, PathBuf};

use crate::core::RpgGraph;
use crate::error::{Result, RpgError};
use crate::parser::ParserRegistry;
use crate::register_parsers;
use crate::storage::RpgStore;

pub use crate::error::ParseFailure;

/// Result of encoding a repository.
#[derive(Debug)]
pub struct EncodeResult {
    /// The generated graph
    pub graph: RpgGraph,
    /// Number of files successfully parsed
    pub files_processed: usize,
    /// Number of files skipped (no parser, unreadable)
    pub files_skipped: usize,
    /// Files that failed to parse
    pub parse_errors: Vec<ParseFailure>,
}

impl EncodeResult {
    /// Returns true if all files were processed successfully
    pub fn is_complete(&self) -> bool {
        self.parse_errors.is_empty() && self.files_skipped == 0
    }

    /// Returns total files encountered
    pub fn total_files(&self) -> usize {
        self.files_processed + self.files_skipped + self.parse_errors.len()
    }
}

/// Main encoder for converting repositories into graphs.
///
/// # Example
///
/// ```no_run
/// use rpg_encoder::RpgEncoder;
/// use std::path::Path;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut encoder = RpgEncoder::new()?;
/// let result = encoder.encode(Path::new("./src"))?;
///
/// println!("Files: {}, Nodes: {}, Edges: {}",
///     result.files_processed,
///     result.graph.node_count(),
///     result.graph.edge_count());
/// # Ok(())
/// # }
/// ```
pub struct RpgEncoder {
    registry: ParserRegistry,
    root: Option<PathBuf>,
    graph: Option<RpgGraph>,
    store: Option<RpgStore>,
}

impl Default for RpgEncoder {
    fn default() -> Self {
        Self::new().expect("Failed to initialize RpgEncoder")
    }
}

impl RpgEncoder {
    /// Create a new encoder with default Rust parser.
    pub fn new() -> Result<Self> {
        let mut registry = ParserRegistry::new();

        // Rust parser is always available
        let parser = crate::languages::RustParser::new()
            .map_err(|e| RpgError::parser_init("rust", e.to_string()))?;
        registry.register(Box::new(parser));

        // All parsers are now always available
        register_parsers!(
            registry,
            crate::languages::PythonParser,
            crate::languages::GoParser,
            crate::languages::CParser,
            crate::languages::CppParser,
            crate::languages::JavaScriptParser,
            crate::languages::TypeScriptParser,
            crate::languages::JavaParser,
            crate::languages::RubyParser,
            crate::languages::LuaParser,
            crate::languages::SwiftParser,
            crate::languages::HaskellParser,
            crate::languages::CSharpParser,
            crate::languages::ScalaParser,
        );

        Ok(Self {
            registry,
            root: None,
            graph: None,
            store: None,
        })
    }

    /// Register a custom parser.
    pub fn with_parser(mut self, parser: Box<dyn crate::parser::LanguageParser>) -> Self {
        self.registry.register(parser);
        self
    }

    /// Register a custom parser (mutable).
    pub fn register_parser(&mut self, parser: Box<dyn crate::parser::LanguageParser>) {
        self.registry.register(parser);
    }

    /// Encode a repository directory into a graph.
    pub fn encode(&mut self, root: &Path) -> Result<EncodeResult> {
        if !root.exists() {
            return Err(RpgError::InvalidPath(format!(
                "Path does not exist: {}",
                root.display()
            )));
        }

        if !root.is_dir() {
            return Err(RpgError::InvalidPath(format!(
                "Path is not a directory: {}",
                root.display()
            )));
        }

        self.root = Some(root.to_path_buf());

        let repo_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repository");

        let mut builder = GraphBuilder::new().with_repo(repo_name, root);

        let walker = FileWalker::new(root);
        let files = walker.walk_with_parser_filter(&self.registry)?;

        tracing::info!("Found {} files to parse", files.len());

        let mut parse_errors = Vec::new();
        let mut files_processed = 0;
        let mut files_skipped = 0;

        for file_path in files {
            let source = match std::fs::read_to_string(&file_path) {
                Ok(s) => s,
                Err(e) => {
                    let err = RpgError::Io(e);
                    parse_errors.push(ParseFailure::from_error(&file_path, &err));
                    files_skipped += 1;
                    continue;
                }
            };

            let parser = match self.registry.get_parser(&file_path) {
                Some(p) => p,
                None => {
                    files_skipped += 1;
                    continue;
                }
            };

            let language = parser.language_name();

            match parser.parse(&source, &file_path) {
                Ok(result) => {
                    builder = builder.try_add_parsed_file(&result, language)?;
                    files_processed += 1;
                }
                Err(e) => {
                    tracing::warn!("Failed to parse {}: {}", file_path.display(), e);
                    parse_errors.push(ParseFailure::from_error(&file_path, &e));
                }
            }
        }

        let graph = builder.link_all().build();
        self.graph = Some(graph.clone());

        tracing::info!(
            processed = files_processed,
            skipped = files_skipped,
            errors = parse_errors.len(),
            nodes = graph.node_count(),
            edges = graph.edge_count(),
            "Encode complete"
        );

        Ok(EncodeResult {
            graph,
            files_processed,
            files_skipped,
            parse_errors,
        })
    }

    /// Get the encoded graph.
    pub fn graph(&self) -> Option<&RpgGraph> {
        self.graph.as_ref()
    }

    /// Get the repository root path.
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Consume the encoder and return the graph.
    pub fn into_graph(self) -> Option<RpgGraph> {
        self.graph
    }

    /// Get the RPG store (if initialized).
    pub fn store(&self) -> Option<&RpgStore> {
        self.store.as_ref()
    }

    /// Get a mutable reference to the RPG store.
    pub fn store_mut(&mut self) -> Option<&mut RpgStore> {
        self.store.as_mut()
    }

    /// Initialize the RPG store for the given repo path.
    pub fn init_store(&mut self, repo_path: &Path) -> Result<&RpgStore> {
        let store = RpgStore::init(repo_path)?;
        self.store = Some(store);
        Ok(self.store.as_ref().unwrap())
    }

    /// Open an existing RPG store.
    pub fn open_store(&mut self, repo_path: &Path) -> Result<&RpgStore> {
        let store = RpgStore::open(repo_path)?;
        self.store = Some(store);
        Ok(self.store.as_ref().unwrap())
    }

    /// Serialize the graph to JSON (pretty-printed).
    pub fn to_json(&self) -> Result<String> {
        let graph = self.graph.as_ref().ok_or(RpgError::NotEncoded)?;
        to_json(graph)
    }

    /// Serialize the graph to compact JSON.
    pub fn to_json_compact(&self) -> Result<String> {
        let graph = self.graph.as_ref().ok_or(RpgError::NotEncoded)?;
        to_json_compact(graph)
    }

    /// List available languages.
    pub fn languages(&self) -> Vec<&str> {
        self.registry.languages()
    }

    /// Encode a repository with semantic enrichment using LLM.
    ///
    /// This method performs the following steps:
    /// 1. Standard encoding (parse source files into graph)
    /// 2. Extract semantic features from each file using LLM
    /// 3. Update graph nodes with extracted features
    /// 4. Optionally run functional abstraction (hierarchy creation)
    ///
    /// # Arguments
    /// * `root` - Path to the repository root
    /// * `config` - Semantic configuration (LLM client, scope, organization mode)
    ///
    /// # Returns
    /// * `EncodeResult` with the enriched graph
    ///
    /// # Example
    /// ```ignore
    /// use rpg_encoder::{RpgEncoder, SemanticConfig, LlmConfig};
    ///
    /// let config = SemanticConfig::new(LlmConfig::default());
    /// let mut encoder = RpgEncoder::new()?;
    /// let result = encoder.encode_with_semantics(Path::new("./src"), config).await?;
    /// ```
    #[cfg(feature = "llm")]
    pub async fn encode_with_semantics(
        &mut self,
        root: &Path,
        config: crate::agents::SemanticConfig,
    ) -> crate::error::Result<EncodeResult> {
        use crate::agents::FeatureExtractor;
        use crate::encoder::functional::{FunctionalAbstraction, LlmOptions};

        // Encode repository
        let mut result = self.encode(root)?;

        // Create feature extractor
        let extractor = FeatureExtractor::new(config.clone())
            .map_err(|e| RpgError::HttpClient(e.to_string()))?;
        // Extract and apply semantic features
        let repo_info = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repository");

        // Collect all extracted features for functional abstraction
        let mut all_organized_features: Vec<crate::agents::OrganizedFeature> = Vec::new();
        let (mut files_enriched, mut total_entities_enriched) = (0usize, 0usize);

        // Get unique file paths from the graph nodes
        let mut seen_paths: std::collections::HashSet<std::path::PathBuf> =
            std::collections::HashSet::new();
        for node in result.graph.nodes() {
            if let Some(ref path) = node.path {
                if path.is_file() {
                    seen_paths.insert(path.clone());
                }
            }
        }

        for file_path in seen_paths {
            // Read file contents
            let code = match std::fs::read_to_string(&file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Extract features based on scope
            let organized = match config.scope {
                crate::agents::ExtractionScope::File => {
                    extractor
                        .extract_and_organize(&code, &file_path, repo_info, "")
                        .await
                }
                crate::agents::ExtractionScope::Module
                | crate::agents::ExtractionScope::Repository => {
                    // For broader scopes, extract then organize by path (simplified)
                    extractor
                        .extract_from_file(&code, &file_path, repo_info)
                        .await
                        .map(|features: Vec<crate::agents::ExtractedFeature>| {
                            features
                                .into_iter()
                                .flat_map(|f| extractor.organize_by_path(&[f], &file_path))
                                .collect()
                        })
                }
            };

            match organized {
                Ok(features) => {
                    for of in &features {
                        // Find matching node in graph and update semantics
                        if let Some(node) =
                            result.graph.find_node_in_file(&file_path, &of.entity_name)
                        {
                            result.graph.update_node_semantics(
                                node.id,
                                of.features.clone(),
                                of.description.clone(),
                                of.feature_path.clone(),
                            );
                            total_entities_enriched += 1;
                        }
                    }
                    all_organized_features.extend(features);
                    files_enriched += 1;
                }
                Err(_e) => {
                    tracing::warn!(
                        "Failed to extract features from {}: {}",
                        file_path.display(),
                        _e
                    );
                }
            }
        }

        // Run functional abstraction for LlmBased organization
        if matches!(
            config.organization,
            crate::agents::OrganizationMode::LlmBased
        ) {
            let mut abstraction = FunctionalAbstraction::new(&mut result.graph);
            let llm_options = LlmOptions::new(extractor.client(), repo_info.to_string());
            let _abstraction_result = abstraction
                .run_with_llm(Some(&llm_options))
                .await
                .map_err(|e| RpgError::HttpClient(e.to_string()))?;
        }

        tracing::info!(
            files_enriched = files_enriched,
            entities_enriched = total_entities_enriched,
            total_features = all_organized_features.len(),
            "Semantic encoding complete"
        );

        // Update the stored graph
        self.graph = Some(result.graph.clone());

        Ok(result)
    }
}
