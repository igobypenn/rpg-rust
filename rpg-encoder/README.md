# rpg-encoder

> Repository Planning Graph encoder for semantic code analysis

Part of the [RPG-Rust](https://github.com/microsoft/rpg-rust) workspace implementing Microsoft's [ZeroRepo](https://arxiv.org/abs/2502.02084) paper.

## Overview

The encoder parses source code into a **Repository Planning Graph (RPG)** — a structured representation capturing:

- **Nodes**: Functions, types, modules, files, directories
- **Edges**: Calls, imports, inheritance, FFI bindings, semantic relationships

## Installation

```toml
[dependencies]
rpg-encoder = "0.1"

[features]
default = []
llm = []           # LLM-based feature extraction
integration = []   # Integration tests
```

## Feature Flags

| Feature | Description | Dependencies |
|---------|-------------|-------------|
| `llm` | LLM-based feature extraction | `reqwest`, `tokio` |
| `integration` | Integration tests | requires llm |

## Feature Flags

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| `semantic` | Semantic embeddings and similarity search | `reqwest`, `tokio` |
| `llm` | LLM-based feature extraction | `reqwest`, `tokio` |
| `incremental` | Snapshot and diff encoding | `sha2` |
| `python` | Python parser | `tree-sitter-python` |
| `go` | Go parser | `tree-sitter-go` |
| `javascript` | JavaScript parser | `tree-sitter-javascript` |
| `typescript` | TypeScript parser | `tree-sitter-typescript` |
| `java` | Java parser | `tree-sitter-java` |
| `c` | C parser | `tree-sitter-c` |
| `cpp` | C++ parser | `tree-sitter-cpp` |
| `ruby` | Ruby parser | `tree-sitter-ruby` |
| `lua` | Lua parser | `tree-sitter-lua` |
| `swift` | Swift parser | `tree-sitter-swift` |
| `haskell` | Haskell parser | `tree-sitter-haskell` |
| `csharp` | C# parser | `tree-sitter-c-sharp` |
| `scala` | Scala parser | `tree-sitter-scala` |
| `all-languages` | All language parsers | All tree-sitter deps |

### Basic Encoding

```rust
use rpg_encoder::RpgEncoder;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut encoder = RpgEncoder::new()?;
    let result = encoder.encode(Path::new("./my-project"))?;
    
    println!("Nodes: {}", result.graph.node_count());
    println!("Edges: {}", result.graph.edge_count());
    
    let json = encoder.to_json()?;
    println!("{}", json);
    
    Ok(())
}
```

### Semantic Enrichment

```rust
use rpg_encoder::{RpgEncoder, EmbeddingConfig, MockEmbeddingClient};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut encoder = RpgEncoder::new()?;
    let config = EmbeddingConfig::default();
    let client = MockEmbeddingClient::new(config.model.dimension);
    
    let result = encoder
        .encode_with_embeddings(Path::new("./my-project"), &client, config)
        .await?;
    
    println!("Enriched {} nodes", result.enrichment_stats.nodes_enriched);
    println!("Found {} clusters", result.clusters.len());
    
    Ok(())
}
```

## Graph Structure

### Node Categories

| Category | Description |
|----------|-------------|
| `Repository` | Root repository node |
| `Directory` | Filesystem directory |
| `File` | Source file |
| `Module` | Module/namespace |
| `Type` | Struct, class, enum, interface |
| `Function` | Function or method |
| `Variable` | Variable declaration |
| `Constant` | Constant value |
| `Field` | Struct/class field |
| `Parameter` | Function parameter |
| `Import` | Import statement |

### Edge Types

| Edge | Description |
|------|-------------|
| `Contains` | Parent-child relationship |
| `Imports` | Import dependency |
| `Calls` | Function call |
| `Extends` | Class inheritance |
| `Implements` | Interface implementation |
| `References` | Symbol reference |
| `DependsOn` | General dependency |
| `FfiBinding` | Foreign function interface |
| `RequiresFeature` | Semantic feature requirement |
| `EnablesFeature` | Semantic feature provider |
| `RelatedFeature` | Semantically related |

## Documentation

- [Getting Started](docs/getting-started.md)
- [Architecture](docs/architecture.md)
- [Semantic Features](docs/semantic-features.md)
- [Configuration](docs/configuration.md)
- [API Reference](docs/api-reference.md)

## Examples

See the `examples/` directory:

- `basic.rs` — Simple encoding
- `visualize.rs` — DOT graph visualization
- `semantic_basic.rs` — Full semantic pipeline
- `semantic_search.rs` — Similarity search

## License

Apache License, Version 2.0
