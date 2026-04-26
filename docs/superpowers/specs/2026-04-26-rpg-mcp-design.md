# rpg-mcp Design Spec

## Overview

`rpg-mcp` is an MCP (Model Context Protocol) server that exposes the rpg-encoder's repository analysis capabilities to LLM clients (Claude Desktop, Cursor, etc.) via the Model Context Protocol.

**Key properties:**
- Rust binary using the `rmcp` SDK (official Rust MCP SDK)
- stdio transport (MCP host spawns as child process)
- Single repo per instance
- Background file watcher with debounced incremental updates via `RpgEvolution`
- Optional LLM-based semantic enrichment
- Runs natively or in Docker

## Architecture

### Directory Layout

```
rpg-rust/
├── rpg-encoder/          # existing
├── rpg-generator/        # existing
├── rpg-mcp/              # NEW crate
│   ├── Cargo.toml
│   ├── Dockerfile
│   ├── src/
│   │   ├── main.rs       # entry point, env vars, stdio serve
│   │   ├── service.rs    # RpgService struct (#[tool_router] impl)
│   │   ├── state.rs      # AppState: graph, store, config, watcher
│   │   └── watcher.rs    # file watcher with debounced incremental updates
│   └── tests/
│       └── mcp_test.rs   # integration tests via rmcp client
├── Cargo.toml            # workspace adds rpg-mcp
```

### Runtime Model

1. Binary starts, reads env vars, initializes state
2. Background file watcher starts monitoring `RPG_WORKSPACE`
3. MCP host (Claude Desktop, Cursor) spawns `rpg-mcp` as child process via stdio
4. Tool calls arrive as JSON-RPC over stdin, responses go to stdout
5. File changes trigger debounced incremental updates to the in-memory graph
6. Logging goes to stderr (MCP requirement)

### Dependencies

```toml
[dependencies]
rpg-encoder = { path = "../rpg-encoder" }
rmcp = { version = "0.6", features = ["server", "macros"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
hex = "0.4"
sha2 = "0.10"
walkdir = "2"
notify = "8"             # file system watcher
```

## Lifecycle & Persistence

### Startup Sequence

```
1. Read env vars (RPG_WORKSPACE, RPG_DATA_DIR, RPG_HASH_MODE, LLM config)
2. Compute directory hash of RPG_WORKSPACE
3. Check if RpgStore exists at {RPG_DATA_DIR}/.rpg/:
   a. Store exists + hash file matches → load graph from disk → done
   b. Store exists + hash mismatch → re-encode workspace → save graph + hash → done
   c. No store → encode workspace → save graph + hash → done
4. Start background file watcher on RPG_WORKSPACE
5. Serve MCP protocol over stdio
```

### File Watcher

- Uses `notify` crate to monitor `RPG_WORKSPACE` recursively
- Debounced 2s (collects events for 2s of quiet before processing)
- Skips `.git/`, `target/`, `node_modules/`, and entries matching `.rpgignore`
- On change batch:
  1. Generate diff of changed files
  2. Apply `RpgEvolution` incrementally to in-memory graph
  3. Write patch to `RpgStore` via `write_patch()`
  4. Update directory hash on disk
  5. Log summary of changes (files added/modified/deleted)
- If `RpgEvolution` fails (e.g., corrupt file), log error and skip that change batch
- Watcher runs on a background tokio task, communicates via channel

### Directory Hashing

Two modes, configured via `RPG_HASH_MODE`:

| Mode | Method | Speed | Accuracy |
|------|--------|-------|----------|
| `mtime` (default) | SHA-256 of sorted `(relative_path, mtime_secs)` pairs | ~ms | May false-positive on touch/checkout |
| `content` | SHA-256 of sorted `(relative_path, file_content_hash)` pairs | ~seconds | Guaranteed accurate |

Hash stored at `{RPG_DATA_DIR}/.rpg/dir_hash` as hex string.

### State Management

- `AppState` holds `RpgGraph` behind `Arc<RwLock<RpgGraph>>`
- File watcher updates graph under write lock
- Tool handlers read graph under read lock
- `RpgStore` writes are serialized (only one writer: the watcher task)

### Persistence

- Uses `RpgStore` from rpg-encoder: base JSON + append-only patches
- Initial encoding or full re-encode writes via `save_base()`
- Incremental updates write via `write_patch()`
- LLM-enriched data stored inline on graph nodes

## MCP Tools

### Tool: `encode_repo`

Manual override to force full re-encoding of the workspace. Replaces current graph and persists new base snapshot.

**Input:** none
**Output:** JSON with encoding stats (files processed, nodes, edges, parse errors, duration)

### Tool: `get_graph_summary`

**Input:** none
**Output:** JSON with node count, edge count, languages, edge type breakdown, import resolution rate

### Tool: `search_nodes`

**Input:**
- `query` (string, required) — substring match on node name
- `kind` (string, optional) — filter by kind (e.g., "fn", "struct", "trait")
- `category` (string, optional) — filter by category (e.g., "function", "type")
- `limit` (int, optional, default 50)

**Output:** JSON array of matching nodes with id, name, kind, path, signature, description

### Tool: `get_node_details`

**Input:**
- `node_id` (string, required)

**Output:** Full node info plus incoming/outgoing edges with target/source node names

### Tool: `get_edges`

**Input:**
- `source_id` (string, optional)
- `target_id` (string, optional)
- `edge_type` (string, optional)
- `limit` (int, optional, default 100)

**Output:** JSON array of edges with source/target node names and metadata

### Tool: `get_skeleton`

**Input:** none
**Output:** JSON tree of repo skeleton (files, units per file, kinds, visibility)

### Tool: `get_features`

**Input:**
- `file_path` (string, optional) — filter to specific file

**Output:** JSON feature tree — entities with features, descriptions, feature paths

### Tool: `get_components`

**Input:** none
**Output:** JSON component plan — architectural components, their nodes, relationships

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `RPG_WORKSPACE` | Yes | — | Path to the repo to encode |
| `RPG_DATA_DIR` | No | `{RPG_WORKSPACE}/.rpg-data` | Persistence directory |
| `RPG_HASH_MODE` | No | `mtime` | `mtime` or `content` |
| `OPENAI_API_KEY` | No | — | LLM API key |
| `OPENAI_BASE_URL` | No | — | LLM base URL |
| `OPENAI_MODEL` | No | `gpt-4o-mini` | LLM model |
| `OPENAI_REASONING` | No | `false` | Enable reasoning mode |
| `RPG_DEBUG` | No | `false` | Debug logging |
| `RPG_SEMANTIC` | No | `false` | Enable LLM semantic enrichment |

## Docker

### Dockerfile

Multi-stage build:
1. `rust:1.82-bookworm` builder — `cargo build --release -p rpg-mcp`
2. `debian:bookworm-slim` runtime — copy binary, set entrypoint

### Usage

```bash
docker run -i --rm \
  -v /path/to/repo:/workspace \
  -v /path/to/persistence:/data \
  -e OPENAI_API_KEY=... \
  -e RPG_SEMANTIC=true \
  rpg-mcp
```

### MCP Host Config (Claude Desktop)

```json
{
  "mcpServers": {
    "rpg": {
      "command": "docker",
      "args": ["run", "-i", "--rm",
        "-v", "/path/to/repo:/workspace",
        "-v", "/path/to/persistence:/data",
        "-e", "OPENAI_API_KEY",
        "rpg-mcp"]
    }
  }
}
```

### Native Binary

```json
{
  "mcpServers": {
    "rpg": {
      "command": "/usr/local/bin/rpg-mcp",
      "env": {
        "RPG_WORKSPACE": "/path/to/repo"
      }
    }
  }
}
```

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Workspace not mounted/empty | Tool error: "Workspace directory is empty or not mounted" |
| LLM errors (429, 401, timeout) | Graph still produced; response notes enrichment was skipped |
| Corrupt persistence | Log warning, delete store, re-encode from scratch |
| Large repos (>10k files) | Queries enforce default `limit`; watcher processes changes incrementally |
| Watcher evolution failure | Log error, skip change batch, graph remains at last good state |
| Hash mismatch on startup | Automatic re-encode; old graph replaced |

## Testing

- **Unit tests:** Tool handlers with mock `AppState` (in-memory graph, no filesystem)
- **Integration tests:** Use `rmcp` client transport to spawn process, call tools, verify responses
- **E2E test:** Encode `rpg-encoder/src/` directory, verify node/edge counts match expectations
- **Watcher tests:** Create temp dir, modify files, verify graph updates
