# Semantic Features

This document covers the semantic embedding and similarity features of RPG Encoder.

## Overview

Semantic features allow you to:
- Enrich nodes with vector embeddings
- Find similar code based on meaning
- Detect semantic relationships between components
- Cluster related functionality

```
+------------------+     +------------------+     +------------------+
|   Source Code    | --> |  EmbeddingClient | --> |  Node.embedding  |
|  + documentation |     |  (text → vector) |     +------------------+
+------------------+     +------------------+              |
                                                              v
+------------------+     +------------------+     +------------------+
| SimilaritySearch | <-- | Cosine Similarity| <-- | Semantic Edges   |
| (find similar)   |     | (compare vectors)|     | (relationships)  |
+------------------+     +------------------+     +------------------+
```

## Enabling Semantic Features

```toml
[dependencies.rpg-encoder]
version = "0.1"
features = ["semantic"]
```

Additional embedding strategies:

```toml
features = ["semantic", "embedding-hybrid", "embedding-multi"]
```

## Embedding Strategies

| Strategy | Description | Use Case |
|----------|-------------|----------|
| `Code` | Source + signature + docs | Find similar implementations |
| `Semantic` | Feature name + description + patterns | Find related concepts |
| `Hybrid` | Weighted code + semantic | General purpose (default) |
| `MultiVector` | Separate code and semantic vectors | Different similarity metrics |

### Code Strategy

Embeds source code content:

```rust
use rpg_encoder::{EmbeddingConfig, EmbeddingStrategy};

let config = EmbeddingConfig::default()
    .with_strategy(EmbeddingStrategy::Code);
```

Text built from:
- Node signature (`pub fn add(a: i32, b: i32) -> i32`)
- Documentation comments
- Source code (if `source_ref` available)

### Semantic Strategy

Embeds metadata only:

```rust
let config = EmbeddingConfig::default()
    .with_strategy(EmbeddingStrategy::Semantic);
```

Text built from:
- Feature name
- Description
- Patterns (async, error_handling, etc.)
- Domain (logic, data, organization)

### Hybrid Strategy (Default)

Combines code and semantic with configurable weights:

```rust
let config = EmbeddingConfig {
    content: ContentConfig {
        source_weight: 0.4,
        semantic_weight: 0.6,
        ..Default::default()
    },
    ..Default::default()
};
```

Output format: `[0.4] <code text> [0.6] <semantic text>`

### MultiVector Strategy

Stores separate embeddings for code and semantic:

```rust
// Requires "embedding-multi" feature
let config = EmbeddingConfig::default()
    .with_strategy(EmbeddingStrategy::MultiVector);
```

Result stored in `Node.semantic.embeddings` with `MultiEmbedding { code, semantic }`.

## Embedding Clients

### MockEmbeddingClient

For testing and development:

```rust
use rpg_encoder::{EmbeddingConfig, MockEmbeddingClient};

let config = EmbeddingConfig::default();
let client = MockEmbeddingClient::new(config.model.dimension);

// Deterministic embeddings based on text hash
let embedding = client.embed("hello world").await?;
```

Properties:
- Deterministic (same text → same embedding)
- Fast (no network calls)
- Normalized vectors
- Configurable dimension

### OpenAICompatibleClient

For production with llama.cpp, Ollama, or OpenAI:

```rust
use rpg_encoder::{EmbeddingConfig, OpenAICompatibleClient};

let config = EmbeddingConfig::default()
    .with_endpoint("http://localhost:8080/v1")
    .with_model("qwen3-embedding");

let client = OpenAICompatibleClient::new(config.model)?;
let embedding = client.embed("hello world").await?;
```

Environment variables:
- `RPGEN_EMBEDDING_ENDPOINT` — API endpoint URL
- `RPGEN_EMBEDDING_MODEL` — Model name

## Semantic Enrichment

### Using the Enricher

```rust
use rpg_encoder::{RpgEncoder, EmbeddingConfig, MockEmbeddingClient};
use rpg_encoder::{SemanticEnricher, SourceCache};

// Option 1: High-level API
let mut encoder = RpgEncoder::new()?;
let client = MockEmbeddingClient::new(1024);
let result = encoder
    .encode_with_embeddings(path, &client, EmbeddingConfig::default())
    .await?;

// Option 2: Low-level API
let mut graph = /* ... */;
let mut cache = SourceCache::new();
cache.load_directory(&repo_path)?;

let enricher = SemanticEnricher::new(EmbeddingConfig::default());
let stats = enricher.enrich_graph(&mut graph, &client, &cache).await?;
```

### Enrichment Stats

```rust
pub struct EnrichmentStats {
    pub nodes_enriched: usize,
    pub embeddings_generated: usize,
    pub errors: Vec<String>,
}
```

### Eligible Node Categories

Only these categories are enriched:
- `Function`
- `Type`
- `Module`
- `Variable`
- `Constant`
- `Field`

## Semantic Edge Detection

After enrichment, detect semantic relationships:

```rust
use rpg_encoder::SemanticEdgeDetector;

let detector = SemanticEdgeDetector::new(&config);
let stats = detector.detect_edges(&mut graph);

println!("RequiresFeature: {}", stats.requires_feature_edges);
println!("EnablesFeature:  {}", stats.enables_feature_edges);
println!("RelatedFeature:  {}", stats.related_feature_edges);
```

### Edge Types

| Edge | Direction | Meaning |
|------|-----------|---------|
| `RequiresFeature` | Consumer → Provider | Needs functionality from target |
| `EnablesFeature` | Provider → Consumer | Provides functionality to target |
| `RelatedFeature` | Bidirectional | Semantically similar |

### Provider vs Consumer

A node is a **provider** if:
- Category is `Type`, `Module`, or `Function`
- Has defined patterns (not empty)

## Functional Clusters

Group related nodes by similarity:

```rust
let clusters = detector.find_functional_clusters(&graph);

for cluster in &clusters {
    println!("Cluster: {} ({} members)", cluster.name, cluster.members.len());
    for node_id in &cluster.members {
        let node = graph.get_node(*node_id).unwrap();
        println!("  - {}", node.name);
    }
}
```

Cluster naming:
1. Common prefix: `auth_login`, `auth_logout` → cluster named `auth`
2. Domain fallback: No common prefix → use domain (`logic_cluster`)

## Similarity Search

Search for similar nodes:

```rust
use rpg_encoder::SimilaritySearch;

let search = SimilaritySearch::new(&graph, 0.7); // threshold

// Search by embedding
let query_embedding = client.embed("authentication function").await?;
let results = search.find_similar(&query_embedding);

for result in results.iter().take(5) {
    println!("{}: {:.3}", result.name, result.similarity);
}

// Search by node
let similar = search.find_similar_to_node(some_node_id);
```

### SearchResult

```rust
pub struct SearchResult {
    pub node_id: NodeId,
    pub name: String,
    pub similarity: f32,
    pub category: NodeCategory,
}
```

## Configuration

### Thresholds

```rust
let config = EmbeddingConfig {
    thresholds: ThresholdConfig {
        similarity_threshold: 0.85,      // For edges
        min_description_length: 10,      // Min chars for description
    },
    ..Default::default()
};
```

### Content Options

```rust
let config = EmbeddingConfig {
    content: ContentConfig {
        include_source: true,
        include_signature: true,
        include_documentation: true,
        include_feature_name: true,
        include_description: true,
        include_patterns: true,
        include_domain: true,
        source_weight: 0.4,
        semantic_weight: 0.6,
    },
    ..Default::default()
};
```

### Model Configuration

```rust
let config = EmbeddingConfig {
    model: ModelConfig {
        provider: "openai_compatible".to_string(),
        endpoint: "http://localhost:8080/v1".to_string(),
        model: "qwen3-embedding".to_string(),
        dimension: 1024,
        batch_size: 32,
        timeout_secs: 30,
    },
    ..Default::default()
};
```

## Example: Full Pipeline

```rust
use rpg_encoder::{
    RpgEncoder, EmbeddingConfig, MockEmbeddingClient,
};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new("./my-project");
    
    // Configure
    let config = EmbeddingConfig::default()
        .with_similarity_threshold(0.8);
    
    // Create client
    let client = MockEmbeddingClient::new(config.model.dimension);
    
    // Encode with semantic features
    let mut encoder = RpgEncoder::new()?;
    let result = encoder
        .encode_with_embeddings(path, &client, config)
        .await?;
    
    // Report
    println!("Nodes enriched: {}", result.enrichment_stats.nodes_enriched);
    println!("Semantic edges: {}", result.edge_stats.total_edges);
    println!("Clusters found: {}", result.clusters.len());
    
    // Show clusters
    for cluster in &result.clusters {
        println!("\n[{}] {} members", cluster.name, cluster.members.len());
    }
    
    Ok(())
}
```
