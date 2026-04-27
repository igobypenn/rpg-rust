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
└── docs/             # Research and RFCs
```

## Features

- **14+ Languages**: Rust, Python, Go, JavaScript, TypeScript, Java, C, C++, Ruby, Lua, Haskell, Scala, Swift, C#
- **FFI Detection**: Cross-language boundaries (`extern "C"`, cgo, JNI, ctypes)
- **Incremental Updates**: Efficient re-encoding of changed files
- **Semantic Enrichment**: Optional LLM-based feature extraction

## Quick Start

### Installation

```toml
[dependencies]
rpg-encoder = "0.1"
# All language parsers are included by default
```

### Basic Usage

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
| `llm` | LLM-powered feature extraction |
| `integration` | Integration tests (requires llm) |
## Documentation

- [rpg-encoder/README.md](rpg-encoder/README.md) - Encoder documentation
- [Architecture](rpg-encoder/docs/architecture.md) - System design
- [API Reference](rpg-encoder/docs/api-reference.md) - Public API

## Examples
```bash
# Basic encoding
cd rpg-encoder
cargo run --example basic ./my-project

# Visualize as DOT graph
cargo run --example visualize ./my-project --output graph.dot

# Semantic enrichment with mock embeddings
cargo run --features llm --example semantic_basic ./my-project

# Similarity search demo
cargo run --features llm --example semantic_search ./my-project
```

```bash
# Core tests
cargo test

# LLM integration
cargo run --features llm --example llm_debug
```

## License

Licensed under the Apache License, Version 2.0 ([LICENSE](LICENSE)).
