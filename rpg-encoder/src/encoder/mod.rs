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
pub use functional::{AbstractionResult, CollectedFeature, FunctionalAbstraction, FunctionalCentroid};
#[cfg(feature = "llm")]
pub use functional::{AbstractionResponse, LlmOptions};
pub use output::{
    serialize_graph, to_json, to_json_compact, SerializedEdge, SerializedGraph, SerializedNode,
};
pub use validation::ValidationReport;
pub use walker::FileWalker;

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

    /// Get the parser registry (for `generate_diff` / `RpgEvolution`).
    pub fn registry(&self) -> &ParserRegistry {
        &self.registry
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

        let mut result = self.encode(root)?;

        let extractor = FeatureExtractor::new(config.clone())
            .map_err(|e| RpgError::HttpClient(e.to_string()))?;
        let repo_info = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repository");

        let mut all_organized_features: Vec<crate::agents::OrganizedFeature> = Vec::new();
        let (mut files_enriched, mut total_entities_enriched) = (0usize, 0usize);

        let mut seen_paths: std::collections::HashSet<std::path::PathBuf> =
            std::collections::HashSet::new();
        for node in result.graph.nodes() {
            if let Some(ref path) = node.path {
                // Node paths are repo-relative — join with root to check
                // existence and read. Without this join, the is_file() check
                // fails when CWD != workspace (the normal case for MCP servers).
                let full_path = root.join(path);
                if full_path.is_file() {
                    seen_paths.insert(full_path);
                }
            }
        }

        // Extract features for all files CONCURRENTLY, bounded by
        // `config.llm.max_concurrent`. The client's semaphore throttles
        // in-flight HTTP requests; buffer_unordered runs up to that many
        // extraction futures at once. Previously this was a serial `for`
        // loop, so max_concurrent had no effect (one request in flight).
        use futures::stream::{self, StreamExt};

        let scope = config.scope;
        let max_concurrent = config.llm.max_concurrent.max(1);

        let enriched: Vec<(std::path::PathBuf, std::result::Result<Vec<crate::agents::OrganizedFeature>, crate::llm::LlmError>)> = stream::iter(seen_paths)
            .map(|file_path| {
                let extractor = &extractor;
                async move {
                    let code = match std::fs::read_to_string(&file_path) {
                        Ok(c) => c,
                        Err(_) => {
                            let display = file_path.display().to_string();
                            return (file_path, Err(crate::llm::LlmError::Api(format!("read failed: {display}"))));
                        }
                    };
                    let organized = match scope {
                        crate::agents::ExtractionScope::File => {
                            extractor.extract_and_organize(&code, &file_path, repo_info, "").await
                        }
                        crate::agents::ExtractionScope::Module
                        | crate::agents::ExtractionScope::Repository => {
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
                    (file_path, organized)
                }
            })
            .buffer_unordered(max_concurrent)
            .collect()
            .await;

        // Graph mutation (matching + update) stays sequential: it borrows the
        // graph mutably and is cheap relative to the LLM calls above.
        for (file_path, organized) in enriched {
            match organized {
                Ok(features) => {
                    // Build a per-file (name -> Vec<NodeId>) index ONCE,
                    // instead of an O(N) graph scan per entity. The matching
                    // rules (exact, last ::-segment, case-insensitive) are
                    // preserved; only the lookup becomes O(1) per name. Owned
                    // keys so the immutable borrow of the graph ends here.
                    let mut by_name: std::collections::HashMap<String, Vec<crate::core::NodeId>> =
                        std::collections::HashMap::new();
                    for n in result.graph.nodes() {
                        if n.path.as_deref() == Some(file_path.as_path()) {
                            by_name.entry(n.name.clone()).or_default().push(n.id);
                        }
                    }

                    for of in &features {
                        // Try exact, then last ::-segment (Type::method -> method).
                        let last_segment = of.entity_name.rsplit("::").next().unwrap_or(&of.entity_name);
                        let mut matched_ids: Vec<crate::core::NodeId> = by_name
                            .get(of.entity_name.as_str())
                            .cloned()
                            .unwrap_or_default();
                        if matched_ids.is_empty() && last_segment != of.entity_name {
                            matched_ids = by_name.get(last_segment).cloned().unwrap_or_default();
                        }
                        // Case-insensitive fallback (rare): scan this file's
                        // names only — bounded by entities in one file.
                        if matched_ids.is_empty() {
                            let nl = of.entity_name.to_ascii_lowercase();
                            let ll = last_segment.to_ascii_lowercase();
                            matched_ids = by_name
                                .iter()
                                .filter(|(k, _)| {
                                    k.to_ascii_lowercase() == nl || k.to_ascii_lowercase() == ll
                                })
                                .flat_map(|(_, v)| v.iter().copied())
                                .collect();
                        }

                        for id in matched_ids {
                            result.graph.update_node_semantics(
                                id,
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

        tracing::info!(
            files_enriched = files_enriched,
            entities_enriched = total_entities_enriched,
            total_features = all_organized_features.len(),
            "Semantic encoding complete"
        );

        // Phase 2: Functional Abstraction.
        // Now that V^L nodes carry semantic features, induce V^H functional
        // centroids and link them via BelongsToFeature edges. Uses the
        // heuristic path-based inducer (no extra LLM call). The LLM-based
        // path (run_with_llm) is available for callers that want it.
        let abstraction = FunctionalAbstraction::new(&mut result.graph).run()?;
        if abstraction.centroids_created > 0 {
            tracing::info!(
                centroids_created = abstraction.centroids_created,
                nodes_linked = abstraction.nodes_linked,
                "Functional abstraction complete"
            );
        }

        self.graph = Some(result.graph.clone());

        Ok(result)
    }

    /// Encode with LLM semantic enrichment **and** vector embeddings.
    ///
    /// Runs `encode_with_semantics` (parse + LLM features + functional
    /// abstraction), then computes embeddings over every embeddable node's text
    /// (metadata + source) and stores them in a sidecar index at
    /// `<root>/.rpg/embeddings.bin`. The graph JSON is untouched — vectors live
    /// in the sidecar, keyed by [`NodeId`].
    ///
    /// Re-embeds only nodes whose embed text changed since the last run
    /// (incremental via a content-hash cache stored alongside the index).
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying semantic encode fails or the
    /// embedding endpoint is unreachable / returns malformed vectors.
    /// Encode with LLM semantic enrichment **and** vector embeddings.
    ///
    /// Runs `encode_with_semantics` (parse + LLM features + functional
    /// abstraction), then computes embeddings over every embeddable node's text
    /// (metadata + source) and stores them in a sidecar index at
    /// `<root>/.rpg/embeddings.bin`. The graph JSON is untouched — vectors live
    /// in the sidecar, keyed by [`NodeId`].
    ///
    /// Re-embeds only nodes whose embed text changed since the last run
    /// (incremental via a content-hash cache stored alongside the index).
    ///
    /// Requires both the `llm` and `embeddings` features (LLM enrichment
    /// produces the features that embeddings vectorize).
    #[cfg(all(feature = "llm", feature = "embeddings"))]
    pub async fn encode_with_embeddings(
        &mut self,
        root: &Path,
        config: crate::agents::SemanticConfig,
        embed_config: crate::embeddings::EmbeddingConfig,
    ) -> crate::error::Result<EncodeResult> {
        // Phase 1 + 2: semantic encode + functional abstraction.
        let result = self.encode_with_semantics(root, config).await?;

        // Phase 3: Embeddings over the finalized graph (low-level + centroids).
        let emb_client = crate::embeddings::EmbeddingClient::new(embed_config.clone())
            .map_err(RpgError::Embedding)?;

        let sidecar = root.join(crate::storage::RPG_DIR).join("embeddings.bin");
        let mut store = if sidecar.exists() {
            // Re-open an existing index (preserves the hash cache for incremental
            // re-embed). The hash cache is rebuilt from the live graph below.
            let index = crate::embeddings::FlatIndex::load(&sidecar)
                .map_err(RpgError::Embedding)?;
            crate::embeddings::EmbeddingStore::new(Box::new(index), sidecar)
        } else {
            crate::embeddings::EmbeddingStore::new(
                Box::new(crate::embeddings::FlatIndex::new(embed_config.dimension)),
                sidecar,
            )
        };

        let stats = store
            .embed_graph(&result.graph, root, &emb_client, embed_config.batch_size)
            .await
            .map_err(RpgError::Embedding)?;

        tracing::info!(
            embedded = stats.embedded,
            skipped_unchanged = stats.skipped_unchanged,
            queued = stats.queued,
            "Embeddings complete"
        );

        Ok(result)
    }
}
