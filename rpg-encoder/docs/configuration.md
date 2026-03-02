# Configuration

RPG Encoder supports multiple configuration sources with a clear precedence order.

## Configuration Sources

| Priority | Source | Location |
|----------|--------|----------|
| 1 (highest) | Environment variables | `RPGEN_*` |
| 2 | Project config | `./.rpg/embedding.yaml` |
| 3 | Global config | `~/.rpg/embedding.yaml` |
| 4 | Defaults | Hardcoded |

## Configuration Files

### Global Config

Location: `~/.rpg/embedding.yaml`

```yaml
strategy: hybrid
model:
  provider: openai_compatible
  endpoint: http://localhost:8080/v1
  model: qwen3-embedding
  dimension: 1024
  batch_size: 32
  timeout_secs: 30

content:
  include_source: true
  include_signature: true
  include_documentation: true
  source_weight: 0.4
  semantic_weight: 0.6

thresholds:
  similarity_threshold: 0.85
  min_description_length: 10

chunking:
  enabled: true
  max_tokens: 8192
  overlap_tokens: 200
```

### Project Config

Location: `./.rpg/embedding.yaml` (in project root)

```yaml
# Override global settings for this project
strategy: code
thresholds:
  similarity_threshold: 0.9
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RPGEN_EMBEDDING_ENDPOINT` | Embedding API endpoint | `http://localhost:8080/v1` |
| `RPGEN_EMBEDDING_MODEL` | Model name | `qwen3-embedding` |

Example:

```bash
export RPGEN_EMBEDDING_ENDPOINT="http://localhost:8080/v1"
export RPGEN_EMBEDDING_MODEL="nomic-embed-text"
```

## Programmatic Configuration

### Basic Setup

```rust
use rpg_encoder::{EmbeddingConfig, EmbeddingStrategy};

let config = EmbeddingConfig::default();
```

### With Strategy

```rust
let config = EmbeddingConfig::default()
    .with_strategy(EmbeddingStrategy::Hybrid);
```

### With Threshold

```rust
let config = EmbeddingConfig::default()
    .with_similarity_threshold(0.9);
```

### With Endpoint

```rust
let config = EmbeddingConfig::default()
    .with_endpoint("http://localhost:8080/v1")
    .with_model("qwen3-embedding");
```

### From Environment

```rust
let config = EmbeddingConfig::from_env()
    .expect("RPGEN_EMBEDDING_ENDPOINT must be set");
```

### Full Customization

```rust
use rpg_encoder::{
    EmbeddingConfig, EmbeddingStrategy,
    ModelConfig, ContentConfig, ThresholdConfig, ChunkingConfig,
};

let config = EmbeddingConfig {
    strategy: EmbeddingStrategy::Hybrid,
    model: ModelConfig {
        provider: "openai_compatible".into(),
        endpoint: "http://localhost:8080/v1".into(),
        model: "qwen3-embedding".into(),
        dimension: 1024,
        batch_size: 16,
        timeout_secs: 60,
    },
    content: ContentConfig {
        include_source: true,
        include_signature: true,
        include_documentation: true,
        include_feature_name: true,
        include_description: true,
        include_patterns: true,
        include_domain: true,
        source_weight: 0.5,
        semantic_weight: 0.5,
    },
    thresholds: ThresholdConfig {
        similarity_threshold: 0.8,
        min_description_length: 5,
    },
    chunking: ChunkingConfig {
        enabled: false,
        max_tokens: 8192,
        overlap_tokens: 200,
    },
};
```

## Loading Configuration

### ConfigLoader

```rust
use rpg_encoder::config::ConfigLoader;

// Load with full precedence
let config = ConfigLoader::load()?;

// Load global only
let config = ConfigLoader::load_global()?;

// Load project only
let config = ConfigLoader::load_project("./")?;

// Merge manually
let base = ConfigLoader::load_global()?;
let override = ConfigLoader::load_project("./")?;
let merged = EmbeddingConfig::merge(base, override);
```

## Configuration Options Reference

### EmbeddingStrategy

| Value | Description |
|-------|-------------|
| `hybrid` | Weighted code + semantic (default) |
| `code` | Source code only |
| `semantic` | Metadata only |
| `multi_vector` | Separate vectors (requires feature) |

### ModelConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `provider` | String | `openai_compatible` | Provider type |
| `endpoint` | String | `http://localhost:8080/v1` | API endpoint |
| `model` | String | `qwen3-embedding` | Model identifier |
| `dimension` | usize | 1024 | Embedding dimension |
| `batch_size` | usize | 32 | Batch size for embeddings |
| `timeout_secs` | u64 | 30 | Request timeout |

### ContentConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `include_source` | bool | true | Include source code |
| `include_signature` | bool | true | Include function signature |
| `include_documentation` | bool | true | Include doc comments |
| `include_feature_name` | bool | true | Include semantic feature name |
| `include_description` | bool | true | Include semantic description |
| `include_patterns` | bool | true | Include semantic patterns |
| `include_domain` | bool | true | Include semantic domain |
| `source_weight` | f32 | 0.4 | Weight for code content |
| `semantic_weight` | f32 | 0.6 | Weight for semantic content |

### ThresholdConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `similarity_threshold` | f32 | 0.85 | Min similarity for edges |
| `min_description_length` | usize | 10 | Min chars for description |

### ChunkingConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | true | Enable chunking |
| `max_tokens` | usize | 8192 | Max tokens per chunk |
| `overlap_tokens` | usize | 200 | Overlap between chunks |

## .rpgignore

Exclude files from encoding:

```
# .rpgignore
target/
node_modules/
*.min.js
*.generated.*
vendor/
```

Pattern syntax:
- `*` — matches any characters except `/`
- `**` — matches any characters including `/`
- `?` — matches single character
- `!` — negates pattern

## Example Configurations

### Minimal Testing

```yaml
strategy: code
model:
  dimension: 256
thresholds:
  similarity_threshold: 0.9
```

### Production with llama.cpp

```yaml
strategy: hybrid
model:
  endpoint: http://localhost:8080/v1
  model: qwen3-embedding
  dimension: 1024
  batch_size: 32
  timeout_secs: 60
content:
  source_weight: 0.5
  semantic_weight: 0.5
thresholds:
  similarity_threshold: 0.8
```

### Code Similarity Focus

```yaml
strategy: code
content:
  include_source: true
  include_signature: true
  include_documentation: true
  include_feature_name: false
  include_description: false
  include_patterns: false
  include_domain: false
thresholds:
  similarity_threshold: 0.85
```
