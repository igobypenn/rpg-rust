# Contributing to rpg-rust

Thank you for your interest in contributing! This guide covers the basics.

## Development Setup

```bash
git clone <repo-url>
cd rpg-rust
cargo build                    # build everything
cargo test                     # run core tests
cargo test --features embeddings  # full test suite
cargo clippy --workspace --all-features -- -D warnings  # must be clean
```

## Project Structure

```
rpg-rust/
├── rpg-encoder/          # Core library: parsing, graph, storage, embeddings
│   ├── src/core/         # Graph data structure (RpgGraph, Node, Edge, NodeId)
│   ├── src/encoder/      # Encode pipeline + builder + functional abstraction
│   ├── src/languages/    # Per-language tree-sitter parsers (14 languages)
│   ├── src/incremental/  # Snapshot diffing + evolution
│   ├── src/storage/      # .rpg/ persistence (base.json + patches)
│   ├── src/embeddings/   # Vector embeddings (FlatIndex + zvec)
│   ├── src/export/       # GraphML/Cypher/DOT export
│   └── src/scip/         # SCIP enrichment
├── rpg-mcp/              # MCP server (30 tools, stdio transport)
│   ├── src/service.rs    # All 30 MCP tool handlers
│   ├── src/tools/        # Shared helpers (format, memory, telemetry, diff)
│   └── src/watcher.rs    # File watcher (auto-refresh on save)
└── .env.example          # Configuration template
```

## How to Add a Language Parser

1. **Create the parser file**: `rpg-encoder/src/languages/yourlang.rs`
2. **Implement `LanguageParser`** using the `define_parser!` macro:
   ```rust
   use crate::define_parser;
   use crate::parser::{LanguageParser, ParseResult};
   use tree_sitter::Parser;

   define_parser!(YourLangParser, "yourlang", &["yl"]);

   impl crate::parser::TreeSitterParser for YourLangParser {
       fn parse_impl(
           source: &str,
           path: &std::path::Path,
           parser: &mut Parser,
       ) -> crate::error::Result<ParseResult> {
           parser.set_language(&tree_sitter_yourlang::LANGUAGE.into())
               .map_err(|e| crate::error::RpgError::parser_init("yourlang", e.to_string()))?;
           // ... walk the tree and extract definitions, calls, imports, etc.
           Ok(result)
       }
   }
   ```
3. **Add the dependency** to `rpg-encoder/Cargo.toml`:
   ```toml
   tree-sitter-yourlang = { workspace = true }
   ```
4. **Register in the encoder**: edit `rpg-encoder/src/encoder/mod.rs` — add to the
   `register_parsers!` macro call in `RpgEncoder::new()`.
5. **Register in the MCP server**: edit `rpg-mcp/src/main.rs` — add a
   `register_parser!` line in `create_parser_registry()`.
6. **Write tests**: add `rpg-encoder/tests/languages/yourlang_test.rs` with at
   minimum: a function definition, a call, an import. Register in `Cargo.toml`.
7. **Add a fixture**: `rpg-encoder/tests/fixtures/yourlang/basic.yl`

> **Important**: Step 5 (MCP registration) is easy to forget — without it, the
> parser works in `rpg-encoder` tests but `rpg-mcp` won't see files with that
> extension.

## How to Add an MCP Tool

1. **Add the tool method** to `rpg-mcp/src/service.rs` inside the
   `#[tool_router] impl RpgService` block:
   ```rust
   #[tool(description = "One-line description of what the tool does")]
   pub async fn your_tool(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
       let query = get_str(&params, "query")
           .ok_or_else(|| McpError::invalid_params("missing 'query'", None))?;
       // ... implement ...
       Ok(CallToolResult::success(vec![Content::text(
           json!({ "result": "..." }).to_string(),
       )]))
   }
   ```
2. **Add to the server instructions** in `get_info()` so agents know about it.
3. **Write tests**: add a test in `rpg-mcp/tests/` that calls the tool and
   verifies the response shape.
4. **Update the README** tool reference table.

## How to Add an Edge Type

1. Add a variant to `EdgeType` in `rpg-encoder/src/core/edge.rs`.
2. Add a match arm in **all** of these places:
   - `rpg-encoder/src/storage/base.rs` → `parse_edge_type()`
   - `rpg-encoder/src/encoder/output.rs` → `serialize_graph()`
   - `rpg-encoder/src/encoder/validation.rs` → `ValidationReport::from_graph()`
   - `rpg-mcp/src/service.rs` → `parse_edge_type()` (the MCP-side parser)
   - `rpg-encoder/src/export/mod.rs` → `dot_edge_style()` and `to_graphml()`
3. Write a test that creates the edge, serializes, deserializes, and verifies.
4. Add a test in `rpg-mcp/tests/tools_edges.rs`.

## Commit Convention

```
<type>(<scope>): <description>

[optional body]
```

Types: `feat`, `fix`, `test`, `docs`, `refactor`, `perf`, `chore`.
Scopes: `encoder`, `mcp`, `parser`, `graph`, `embeddings`, `storage`, etc.

Examples:
```
feat(mcp): add vector_search tool with embedding similarity
fix(encoder): preserve NodeId through save/reload cycle
test(graph): add property test for bfs_reachable
docs(mcp): update README with Claude Desktop config
```

## Testing Guidelines

- **Unit tests** go in `#[cfg(test)] mod tests` within the source file.
- **Integration tests** go in `tests/` directories, registered in `Cargo.toml`.
- **Property tests** use `proptest` — see `rpg-encoder/tests/property/`.
- **Snapshot tests** use `insta` — see `rpg-mcp/tests/tools_snapshot.rs`.
- Every new feature must have at least one test. Every bug fix must have a
  regression test.
- `cargo clippy --workspace --all-features -- -D warnings` must pass (zero
  warnings allowed).
