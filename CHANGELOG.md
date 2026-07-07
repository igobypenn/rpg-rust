# Changelog

All notable changes to rpg-rust are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- CONTRIBUTING.md with extension guides (parsers, tools, edge types)
- MSRV policy (rust-version = "1.85")
- Config validation: RPG_WORKSPACE must exist, RPG_SEMANTIC warns if no API key
- Case-insensitive boolean env-var parsing (accepts true/1/yes/on)

### Fixed
- Documentation drift: README referenced nonexistent docs/ dir, examples, integration feature
- Inconsistent boolean parsing: RPG_SEMANTIC=TRUE now accepted (was rejected)
- HashMode::from_str now case-insensitive

## [0.1.0] — 2026-07

### Added — MCP Server (rpg-mcp)
- 30 MCP tools: structural search, semantic search, vector search, FFI detection,
  dead code, architecture overview, diff analysis, agent memory, graph export,
  SCIP enrichment, incremental re-encoding
- stdio transport, file watcher (2s debounce), parking_lot locks
- Committed-graph workflow: repo-relative paths, portable .rpg/ directory
- Comprehensive README with client configs (Claude Desktop, Cursor, opencode, ZCode, Docker)
- Competitive analysis (20+ competitors surveyed)
- 24 parity tests + MCP critical-path benchmark

### Added — Embeddings (rpg-encoder)
- FlatIndex (pure Rust, brute-force cosine, binary sidecar persistence)
- zvec backend (Alibaba's in-process vector DB, behind feature gate)
- EmbeddingClient (OpenAI-compatible /embeddings endpoint)
- Dual-mode embedding config (decoupled from llm feature)
- node_embed_text: metadata + source per node, char-boundary-safe truncation

### Added — Graph & Storage
- RpgGraph custom serde rebuilds node_id_map on deserialization
- NodeId preservation across save/reload (embeddings stay valid)
- add_node_preserving_id for stable IDs through BaseSnapshot round-trip
- Graph export: GraphML, Neo4j Cypher, Graphviz DOT
- SCIP enrichment: post-parse edge refinement with provenance metadata
- in_degree() / has_incoming_of_types() non-allocating graph primitives
- bfs_reachable() canonical BFS helper
- Node::line_range() shared helper
- Display impls for NodeCategory, EdgeType, NodeLevel

### Added — Incremental Evolution
- Cross-file edge re-linking after incremental update (pending_calls + relink)
- UnitType::from_kind accepts all parser kind strings (fn, method, class, etc.)
- Wildcard import parsing (is_glob field, resolution is a known limitation)

### Fixed — Architectural Issues
- Store path double-nesting (main.rs passed data_dir to RpgStore which appends .rpg)
- Lock-ordering deadlock in detect_changes (snapshot.write then graph.write)
- Watcher degrades gracefully on OS inotify limit
- Watcher task survives inner panics
- Watcher shutdown pattern fixed (recv None on close)
- explore_graph limit bounds BFS traversal
- apply_patch preserves NodeIds + applies edge deltas
- extract_unit_content slice panic on truncated files
- MemoryStore same-process tmp file race
- FlatIndex dimension mismatch defensive handling
- Semaphore::new(0) hang on zero max_concurrent
- encode_with_semantics relative path fix (join root)
- dir_hash persistence on empty-diff early return
- LLM debug preview byte-slice panic on multi-byte UTF-8
- NodeId overflow on crafted embeddings file
- Diff parser handles spaced file paths and \ No newline marker
- DOT export escapes newlines; Cypher uses sequential MATCH

### Removed
- utils/similarity.rs (dead code, zero callers)
- DualEmbedder (referenced nonexistent feature)
- Dead RpgError variants (EmptyResponse)
- TypeRefKind::Bound (zero usages)
- LlmProvider enum + provider field (written never read)
- integration feature (pure alias for llm)
- scip-parse feature (broken, 6 API mismatches)
- 16 internal-only crate-root re-exports

### Performance
- get_skeleton: O(F×E) → O(F×degree) via edges_from
- get_architecture_overview: non-allocating in_degree() instead of edges_to().len()
- find_dead_code: non-allocating has_incoming_of_types() short-circuit
- semantic_search: eliminated format!("{:?}", enum) allocations
- All MCP interactive tools <4ms at 1000 nodes (benchmarked)

### Test Suite
- 645 tests, 0 failures, 0 clippy warnings
- 14 regression tests for deferred architectural issues
- 24 parity tests (competitive capability verification)
- MCP critical-path benchmark (6 tools × 2 scales)
- Cross-machine portability test
- Property-based tests for graph invariants
