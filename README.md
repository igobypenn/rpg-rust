# RPG-Rust

[![CI](https://github.com/microsoft/rpg-rust/workflows/CI/badge.svg)](https://github.com/microsoft/rpg-rust/actions)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

> Rust implementation of **Repository Planning Graphs** from Microsoft's [ZeroRepo](https://arxiv.org/abs/2502.02084) paper

A **Repository Planning Graph (RPG)** is a structured representation of code that captures both syntactic structure (functions, types, modules) and semantic relationships (calls, imports, inheritance, FFI bindings). This workspace provides:

- **rpg-encoder**: Parse codebases into RPG format
- **rpg-mcp**: MCP server for RPG-based code intelligence

## About ZeroRepo

This project implements the concepts from Microsoft Research's ZeroRepo paper, which introduces Repository Planning Graphs as a unified representation for:

- Code understanding and navigation
- Semantic search across codebases
- Multi-language FFI analysis
- LLM-based code generation

**Paper**: [ZeroRepo: Repository Planning Graphs for Code Understanding](https://arxiv.org/abs/2502.02084)

## Workspace Structure

```
rpg-rust/
├── rpg-encoder/      # Code → RPG (analysis)
├── rpg-mcp/          # MCP server for code intelligence
└── rpg-encoder/docs/ # Architecture, API reference, configuration
```

## Features

- **14+ Languages**: Rust, Python, Go, JavaScript, TypeScript, Java, C, C++, Ruby, Lua, Haskell, Scala, Swift, C#
- **FFI Detection**: Cross-language boundaries (`extern "C"`, cgo, JNI, ctypes)
- **Incremental Updates**: Efficient re-encoding of changed files
- **Semantic Enrichment**: Optional LLM-based feature extraction
- **Vector Embeddings**: FlatIndex + optional zvec backend for semantic search
- **30 MCP Tools**: Structural search, relationship traversal, impact analysis,
  dead code detection, architecture overview, diff analysis, agent memory,
  graph export, SCIP enrichment

## Quick Start

### MCP Server (rpg-mcp)

See [rpg-mcp/README.md](rpg-mcp/README.md) for full setup and client configuration
(Claude Desktop, Cursor, opencode, ZCode, Docker).

```bash
cargo build --release -p rpg-mcp
export RPG_WORKSPACE=/path/to/your/repo
./target/release/rpg-mcp
```

### Encoder Library (rpg-encoder)

```toml
[dependencies]
rpg-encoder = "0.1"
# All language parsers are included by default
```

```rust
use rpg_encoder::RpgEncoder;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut encoder = RpgEncoder::new()?;
    let result = encoder.encode(Path::new("./my-project"))?;

    println!("Nodes: {}", result.graph.node_count());
    println!("Edges: {}", result.graph.edge_count());

    Ok(())
}
```

## Feature Flags

| Flag | Description |
|------|-------------|
| `llm` | LLM-powered semantic feature extraction |
| `embeddings` | Vector embeddings (FlatIndex + optional zvec backend) |
| `zvec` | zvec vector DB backend (native C++ dependency) |

## Documentation

- [rpg-mcp/README.md](rpg-mcp/README.md) - MCP server setup, client configs, tool reference
- [rpg-encoder/docs/architecture.md](rpg-encoder/docs/architecture.md) - System design
- [rpg-encoder/docs/api-reference.md](rpg-encoder/docs/api-reference.md) - Public API
- [rpg-mcp/COMPETITIVE_ANALYSIS.md](rpg-mcp/COMPETITIVE_ANALYSIS.md) - Competitive landscape

## Examples
```bash
# Basic encoding
cd rpg-encoder
cargo run --example basic ./my-project

# Visualize as DOT graph
cargo run --example visualize ./my-project --output graph.dot

# LLM debug
cargo run --features llm --example llm_debug

# Embedding benchmark (FlatIndex vs zvec)
cargo run --release --features zvec --example bench_embeddings -- --n 1000
```

## Testing

```bash
# Core tests (no features needed)
cargo test

# Full test suite (includes embeddings, LLM tests)
cargo test --features embeddings

# MCP server tests
cargo test -p rpg-mcp

# Clippy (must be clean)
cargo clippy --workspace --all-features -- -D warnings
```

## License

Licensed under the Apache License, Version 2.0 ([LICENSE](LICENSE)).
