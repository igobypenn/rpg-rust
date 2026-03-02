# API Reference

This document provides an overview of the public API.

## Core Types

### Node

```rust
pub struct Node {
    pub id: NodeId,
    pub category: NodeCategory,
    pub kind: String,
    pub language: String,
    pub name: String,
    pub path: Option<PathBuf>,
    pub location: Option<SourceLocation>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub description: Option<String>,
    pub features: Vec<String>,
    pub signature: Option<String>,
    pub documentation: Option<String>,
    pub source_ref: Option<SourceRef>,
    #[cfg(feature = "semantic")]
    pub semantic: Option<SemanticData>,
}

impl Node {
    pub fn new(id: NodeId, category: NodeCategory, kind: &str, language: &str, name: &str) -> Self;
    pub fn with_path(self, path: impl Into<PathBuf>) -> Self;
    pub fn with_location(self, location: SourceLocation) -> Self;
    pub fn with_description(self, description: impl Into<String>) -> Self;
    pub fn with_signature(self, signature: impl Into<String>) -> Self;
    pub fn with_documentation(self, documentation: impl Into<String>) -> Self;
    pub fn with_source_ref(self, start_line: usize, end_line: usize) -> Self;
}
```

### NodeCategory

```rust
pub enum NodeCategory {
    Repository,
    Directory,
    File,
    Module,
    Type,
    Function,
    Variable,
    Import,
    Constant,
    Field,
    Parameter,
    Feature,
    Component,
}
```

### Edge

```rust
pub struct Edge {
    pub edge_type: EdgeType,
    pub metadata: HashMap<String, serde_json::Value>,
}

pub enum EdgeType {
    Contains,
    Imports,
    Calls,
    Extends,
    Implements,
    References,
    DependsOn,
    FfiBinding,
    Defines,
    Uses,
    UsesType,
    ImplementsFeature,
    BelongsToComponent,
    #[cfg(feature = "semantic")]
    RequiresFeature,
    #[cfg(feature = "semantic")]
    EnablesFeature,
    #[cfg(feature = "semantic")]
    RelatedFeature,
}
```

### RpgGraph

```rust
pub struct RpgGraph { /* ... */ }

impl RpgGraph {
    pub fn new() -> Self;
    pub fn add_node(&mut self, node: Node) -> NodeId;
    pub fn add_edge(&mut self, source: NodeId, target: NodeId, edge: Edge) -> EdgeId;
    pub fn add_typed_edge(&mut self, source: NodeId, target: NodeId, edge_type: EdgeType) -> EdgeId;
    pub fn get_node(&self, id: NodeId) -> Option<&Node>;
    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut Node>;
    pub fn nodes(&self) -> impl Iterator<Item = &Node>;
    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node>;
    pub fn edges(&self) -> impl Iterator<Item = (NodeId, NodeId, &Edge)>;
    pub fn node_count(&self) -> usize;
    pub fn edge_count(&self) -> usize;
    pub fn find_node_by_path(&self, path: &Path) -> Option<&Node>;
    pub fn find_node_by_name(&self, name: &str, category: Option<NodeCategory>) -> Option<&Node>;
    pub fn find_node_by_location(&self, file_path: &Path, line: usize) -> Option<&Node>;
    pub fn children_of(&self, parent_id: NodeId) -> Vec<&Node>;
    pub fn to_petgraph(&self) -> DiGraph<Node, Edge>;
}
```

## Encoder

### RpgEncoder

```rust
pub struct RpgEncoder { /* ... */ }

impl RpgEncoder {
    pub fn new() -> Result<Self>;
    pub fn with_parser(self, parser: Box<dyn LanguageParser>) -> Self;
    pub fn register_parser(&mut self, parser: Box<dyn LanguageParser>);
    pub fn encode(&mut self, root: &Path) -> Result<EncodeResult>;
    
    #[cfg(feature = "semantic")]
    pub async fn encode_with_embeddings<C: EmbeddingClient>(
        &mut self,
        root: &Path,
        client: &C,
        config: EmbeddingConfig,
    ) -> Result<SemanticEncodeResult>;
    
    pub fn graph(&self) -> Option<&RpgGraph>;
    pub fn into_graph(self) -> Option<RpgGraph>;
    pub fn to_json(&self) -> Result<String>;
    pub fn to_json_compact(&self) -> Result<String>;
    pub fn languages(&self) -> Vec<&str>;
}
```

### EncodeResult

```rust
pub struct EncodeResult {
    pub graph: RpgGraph,
    pub files_processed: usize,
    pub files_skipped: usize,
    pub parse_errors: Vec<ParseFailure>,
}

pub struct ParseFailure {
    pub path: PathBuf,
    pub error: String,
}
```

### SemanticEncodeResult

```rust
#[cfg(feature = "semantic")]
pub struct SemanticEncodeResult {
    pub graph: RpgGraph,
    pub files_processed: usize,
    pub files_skipped: usize,
    pub parse_errors: Vec<ParseFailure>,
    pub enrichment_stats: EnrichmentStats,
    pub edge_stats: EdgeDetectionStats,
    pub clusters: Vec<FunctionalCluster>,
}
```

## Embedding

### EmbeddingClient

```rust
#[async_trait]
pub trait EmbeddingClient: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    fn dimension(&self) -> usize;
}
```

### EmbeddingConfig

```rust
pub struct EmbeddingConfig {
    pub strategy: EmbeddingStrategy,
    pub model: ModelConfig,
    pub content: ContentConfig,
    pub thresholds: ThresholdConfig,
    pub chunking: ChunkingConfig,
}

impl EmbeddingConfig {
    pub fn from_env() -> Option<Self>;
    pub fn with_strategy(self, strategy: EmbeddingStrategy) -> Self;
    pub fn with_similarity_threshold(self, threshold: f32) -> Self;
    pub fn with_endpoint(self, endpoint: impl Into<String>) -> Self;
    pub fn with_model(self, model: impl Into<String>) -> Self;
    pub fn merge(base: Self, override_config: Self) -> Self;
}
```

### EmbeddingStrategy

```rust
pub enum EmbeddingStrategy {
    Hybrid,
    Code,
    Semantic,
    #[cfg(feature = "embedding-multi")]
    MultiVector,
}
```

### MockEmbeddingClient

```rust
pub struct MockEmbeddingClient { /* ... */ }

impl MockEmbeddingClient {
    pub fn new(dimension: usize) -> Self;
}
```

### OpenAICompatibleClient

```rust
pub struct OpenAICompatibleClient { /* ... */ }

impl OpenAICompatibleClient {
    pub fn new(config: ModelConfig) -> Result<Self>;
}
```

## Semantic Analysis

### SemanticEnricher

```rust
pub struct SemanticEnricher { /* ... */ }

impl SemanticEnricher {
    pub fn new(config: EmbeddingConfig) -> Self;
    pub async fn enrich_graph<C: EmbeddingClient>(
        &self,
        graph: &mut RpgGraph,
        client: &C,
        source_cache: &SourceCache,
    ) -> Result<EnrichmentStats>;
}
```

### SemanticEdgeDetector

```rust
pub struct SemanticEdgeDetector { /* ... */ }

impl SemanticEdgeDetector {
    pub fn new(config: &EmbeddingConfig) -> Self;
    pub fn with_thresholds(self, similarity: f32, related: f32) -> Self;
    pub fn with_max_edges_per_node(self, max: usize) -> Self;
    pub fn detect_edges(&self, graph: &mut RpgGraph) -> EdgeDetectionStats;
    pub fn find_functional_clusters(&self, graph: &RpgGraph) -> Vec<FunctionalCluster>;
}
```

### SimilaritySearch

```rust
pub struct SimilaritySearch<'a> { /* ... */ }

impl<'a> SimilaritySearch<'a> {
    pub fn new(graph: &'a RpgGraph, threshold: f32) -> Self;
    pub fn find_similar(&self, query_embedding: &[f32]) -> Vec<SearchResult>;
    pub fn find_similar_to_node(&self, node_id: NodeId) -> Vec<SearchResult>;
}

pub struct SearchResult {
    pub node_id: NodeId,
    pub name: String,
    pub similarity: f32,
    pub category: NodeCategory,
}
```

### Supporting Types

```rust
pub struct EnrichmentStats {
    pub nodes_enriched: usize,
    pub embeddings_generated: usize,
    pub errors: Vec<String>,
}

pub struct EdgeDetectionStats {
    pub nodes_with_embeddings: usize,
    pub total_edges: usize,
    pub requires_feature_edges: usize,
    pub enables_feature_edges: usize,
    pub related_feature_edges: usize,
}

pub struct FunctionalCluster {
    pub name: String,
    pub members: Vec<NodeId>,
}

pub struct SourceCache { /* ... */ }

impl SourceCache {
    pub fn new() -> Self;
    pub fn with_root(root: PathBuf) -> Self;
    pub fn get(&self, path: &PathBuf) -> Result<String>;
    pub fn load(&mut self, path: &PathBuf) -> Result<()>;
    pub fn load_directory(&mut self, dir: &PathBuf) -> Result<usize>;
    pub fn clear(&mut self);
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32;
```

## Parser

### LanguageParser

```rust
pub trait LanguageParser: Send + Sync {
    fn language_name(&self) -> &str;
    fn file_extensions(&self) -> &[&str];
    fn parse(&self, source: &str, path: &Path) -> Result<ParseResult>;
}
```

### ParserRegistry

```rust
pub struct ParserRegistry { /* ... */ }

impl ParserRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, parser: Box<dyn LanguageParser>);
    pub fn get_parser(&self, path: &Path) -> Option<&dyn LanguageParser>;
    pub fn has_parser(&self, path: &Path) -> bool;
    pub fn languages(&self) -> Vec<&str>;
}
```

## Serialization

```rust
pub fn to_json(graph: &RpgGraph) -> Result<String>;
pub fn to_json_compact(graph: &RpgGraph) -> Result<String>;
pub fn serialize_graph(graph: &RpgGraph) -> Result<SerializedGraph>;

pub struct SerializedGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<SerializedEdge>,
}
```

## Error Handling

```rust
pub type Result<T> = std::result::Result<T, RpgError>;

pub enum RpgError {
    IoError(String),
    ParseError(String),
    ParserInit(String),
    InvalidPath(String),
    NotEncoded,
    Serialization(String),
    Other(String),
}
```

## Feature-Gated Exports

```rust
// Base
pub use core::{Edge, EdgeType, Node, NodeCategory, NodeId, RpgGraph, SourceLocation};
pub use encoder::{to_json, to_json_compact, EncodeResult, GraphBuilder, ParseFailure, RpgEncoder};
pub use error::{Result, RpgError};
pub use parser::{LanguageParser, ParserRegistry};

// semantic feature
pub use embedding::{
    ChunkingConfig, ContentConfig, EmbeddingClient, EmbeddingConfig, EmbeddingStrategy,
    EmbeddingTextBuilder, MockEmbeddingClient, ModelConfig, OpenAICompatibleClient,
    SemanticNodeData, ThresholdConfig,
};
pub use config::{ConfigLoader, RpgConfig};
pub use encoder::{
    EdgeDetectionStats, EnrichmentStats, FunctionalCluster, SemanticEdgeDetector,
    SemanticEncodeResult, SemanticEnricher, SearchResult, SimilaritySearch, SourceCache,
};

// llm feature
pub use llm::{LlmConfig, LlmError, LlmProvider, OpenAIClient};
pub use agents::{ComponentOrganizer, ExtractionScope, FeatureExtractor, SemanticConfig, ...};

// incremental feature
pub use incremental::{
    compute_hash, generate_diff, CachedUnit, CodeUnit, DiffStats, RpgEvolution,
    RpgSnapshot, SnapshotStats, ...
};
```
