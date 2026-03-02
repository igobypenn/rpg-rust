# Getting Started

This guide walks you through encoding your first repository with RPG Encoder.

## Prerequisites

- Rust 1.70 or later
- A codebase to analyze

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
rpg-encoder = "0.1"
```

For semantic features:

```toml
[dependencies.rpg-encoder]
version = "0.1"
features = ["semantic"]
```

## Basic Encoding

The simplest way to encode a repository:

```rust
use rpg_encoder::RpgEncoder;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut encoder = RpgEncoder::new()?;
    let result = encoder.encode(Path::new("./my-project"))?;
    
    println!("Files processed: {}", result.files_processed);
    println!("Files skipped:   {}", result.files_skipped);
    println!("Parse errors:    {}", result.parse_errors.len());
    println!("Nodes:           {}", result.graph.node_count());
    println!("Edges:           {}", result.graph.edge_count());
    
    Ok(())
}
```

## Understanding the Result

The `EncodeResult` contains:

| Field | Type | Description |
|-------|------|-------------|
| `graph` | `RpgGraph` | The encoded graph |
| `files_processed` | `usize` | Successfully parsed files |
| `files_skipped` | `usize` | Files without parsers |
| `parse_errors` | `Vec<ParseFailure>` | Failed parses with errors |

## Working with the Graph

### Iterating Nodes

```rust
for node in result.graph.nodes() {
    println!("[{:?}] {} - {}", node.category, node.name, node.language);
}
```

### Iterating Edges

```rust
for (source, target, edge) in result.graph.edges() {
    let src = result.graph.get_node(source).unwrap();
    let tgt = result.graph.get_node(target).unwrap();
    println!("{} --{:?}--> {}", src.name, edge.edge_type, tgt.name);
}
```

### Finding Nodes

```rust
use rpg_encoder::NodeCategory;

// Find by name
if let Some(node) = result.graph.find_node_by_name("main", Some(NodeCategory::Function)) {
    println!("Found main function: {:?}", node.location);
}

// Find by path
use std::path::PathBuf;
if let Some(node) = result.graph.find_node_by_path(&PathBuf::from("src/main.rs")) {
    println!("Found file node");
}
```

## JSON Output

Serialize the graph to JSON:

```rust
// Pretty-printed
let json = encoder.to_json()?;

// Compact
let compact = encoder.to_json_compact()?;
```

Example output:

```json
{
  "nodes": [
    {
      "id": 0,
      "category": "repository",
      "kind": "repository",
      "language": "rpg",
      "name": "my-project"
    },
    {
      "id": 1,
      "category": "file",
      "kind": "file",
      "language": "rust",
      "name": "main.rs",
      "path": "src/main.rs"
    }
  ],
  "edges": [
    {"source": 0, "target": 1, "edge_type": "contains"}
  ]
}
```

## Adding Language Parsers

Rust is included by default. Enable other languages via features:

```toml
[dependencies.rpg-encoder]
version = "0.1"
features = ["python", "go", "javascript", "typescript"]
```

Or all at once:

```toml
features = ["all-languages"]
```

## Next Steps

- [Architecture](architecture.md) — Understand the system design
- [Semantic Features](semantic-features.md) — Add embedding-based analysis
- [Configuration](configuration.md) — Customize encoding behavior
- [API Reference](api-reference.md) — Full API documentation
