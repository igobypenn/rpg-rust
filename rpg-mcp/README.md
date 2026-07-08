# rpg-mcp

Code graph intelligence for AI coding agents. Analyzes your repository into a
[Repository Planning Graph](https://github.com/microsoft/RPG-ZeroRepo) — a
semantic graph of functions, types, calls, features, and FFI bindings — then
serves it to any MCP-compatible client via 30 query tools.

**Unique capabilities no other code-graph MCP server offers:**
- **LLM-extracted semantic features** — search by *what code does*, not just by name
- **FFI / cross-language edge detection** — `extern "C"`, `#[no_mangle]`, JNI bindings
- **Functional feature tree** — V^H behavioral centroids over V^L implementation nodes
- **Committed-graph workflow** — encode once, commit `.rpg/`, share the graph across the team with zero LLM cost

---

## Quick start (2 minutes)

```sh
# 1. Build
cargo build --release -p rpg-mcp

# 2. Point at your repo
export RPG_WORKSPACE=/path/to/your/repo

# 3. Run (encodes on first launch, then serves via stdio)
./target/release/rpg-mcp
```

The server reads `RPG_WORKSPACE`, parses your codebase (14 languages), builds the
graph, and starts listening on stdin/stdout for MCP protocol messages. The graph
is persisted to `<workspace>/.rpg/` and reused on subsequent launches.

---

## Connecting to your editor or AI tool

rpg-mcp uses **stdio transport** — your editor spawns it as a subprocess and
communicates over its stdin/stdout. Below are exact configs for common clients.

### Claude Desktop

Edit `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS)
or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

```jsonc
{
  "mcpServers": {
    "rpg-mcp": {
      "command": "/absolute/path/to/rpg-mcp",
      "env": {
        "RPG_WORKSPACE": "/absolute/path/to/your/repo"
      }
    }
  }
}
```

Restart Claude Desktop. The `rpg-mcp` tools appear in the tool palette.

### Cursor

Edit `~/.cursor/mcp.json` (or `<project>/.cursor/mcp.json` for project-scoped):

```jsonc
{
  "mcpServers": {
    "rpg-mcp": {
      "command": "/absolute/path/to/rpg-mcp",
      "env": {
        "RPG_WORKSPACE": "/absolute/path/to/your/repo"
      }
    }
  }
}
```

### opencode

Add to your `opencode.json` config:

```jsonc
{
  "mcp": {
    "rpg-mcp": {
      "type": "stdio",
      "command": "/absolute/path/to/rpg-mcp",
      "env": {
        "RPG_WORKSPACE": "/absolute/path/to/your/repo"
      }
    }
  }
}
```

### ZCode

Add rpg-mcp as an MCP server in your ZCode configuration:

```jsonc
{
  "mcpServers": {
    "rpg-mcp": {
      "command": "/absolute/path/to/rpg-mcp",
      "env": {
        "RPG_WORKSPACE": "/absolute/path/to/your/repo"
      }
    }
  }
}
```

### Generic MCP client (any stdio client)

```jsonc
{
  "mcpServers": {
    "rpg-mcp": {
      "command": "/absolute/path/to/rpg-mcp",
      "args": [],
      "env": {
        "RPG_WORKSPACE": "/absolute/path/to/your/repo",
        "RUST_LOG": "info"
      }
    }
  }
}
```

### Docker (isolated, reproducible)

```jsonc
{
  "mcpServers": {
    "rpg-mcp": {
      "command": "docker",
      "args": [
        "run", "--rm", "-i",
        "--volume", "/path/to/repo:/repo",
        "--env", "RPG_WORKSPACE=/repo",
        "rpg-mcp:latest"
      ]
    }
  }
}
```

Build the image first: `docker build -t rpg-mcp:latest ./rpg-mcp`

---

## How source directory mounting works

```
your-repo/
├── src/                    ← your code (watched)
├── .rpg/                   ← rpg-mcp data (auto-created)
│   ├── base.json           ← the serialized graph (committable!)
│   ├── embeddings.bin      ← vector embeddings (committable!)
│   ├── memories.jsonl      ← agent memory notes
│   ├── dir_hash            ← change-detection hash
│   └── manifest.json       ← store version info
├── .env                    ← optional config (see below)
└── ...
```

### `RPG_WORKSPACE`

**Required.** The root directory of the repository to analyze. Must be an
absolute path. All node paths in the graph are stored **repo-relative** so the
`.rpg/` directory is portable across machines (you can commit it).

### File watcher

On startup, rpg-mcp watches `RPG_WORKSPACE` recursively for file changes. When
a file is modified, the watcher:

1. **Debounces** for 2 seconds (coalesces rapid saves)
2. Computes a diff (added/deleted/modified files)
3. Re-parses only the changed files
4. Incrementally updates the graph (cross-file edges are re-linked)
5. Persists the updated snapshot to `.rpg/base.json`

The watcher skips `.rpg/`, `.git/`, `target/`, `node_modules/`, `.next/`,
`dist/`, and `build/` directories.

### `detect_changes` tool

Even without the watcher, you can call the `detect_changes` MCP tool to
on-demand check for file changes and incrementally re-encode. This is useful
for agents that want to refresh the graph before a query.

---

## Semantic encoding + embeddings (optional, requires LLM)

By default, rpg-mcp builds a **structural graph** (functions, types, calls,
contains, imports). To unlock the semantic features (behavioral search, feature
trees, vector embeddings), enable LLM enrichment:

### Step 1: Configure an LLM endpoint

```sh
# .env in your repo root
OPENAI_API_KEY=your-key-here
OPENAI_BASE_URL=https://api.z.ai/api/coding/paas/v4   # or any OpenAI-compatible API
OPENAI_MODEL=GLM-5.2                                    # or gpt-4o, llama-3, etc.
OPENAI_MAX_CONCURRENT=8
```

### Step 2: Enable semantic mode

```sh
RPG_SEMANTIC=true
```

With `RPG_SEMANTIC=true`, the server enriches each function/type with
LLM-extracted behavioral features on first encode. This costs one LLM call per
file (batched concurrently). The features are persisted to `.rpg/base.json` —
subsequent launches reuse them with zero LLM cost.

### Step 3 (optional): Enable vector embeddings

```sh
RPGEN_EMBEDDING_ENDPOINT=http://your-embedding-server:8994/v1
RPGEN_EMBEDDING_MODEL=Qwen3-Embedding-8B
RPGEN_EMBEDDING_DIMENSION=4096
RPGEN_EMBEDDING_MAX_CONCURRENT=8   # independent from OPENAI_MAX_CONCURRENT
```

After encoding, call the `encode_embeddings` MCP tool to compute vector
embeddings over each node's text (metadata + source). Then use `vector_search`
for semantic similarity queries ("find code that does X").

The embeddings are stored in `.rpg/embeddings.bin` and are also committable.

---

## Committed-graph workflow (team sharing)

The key insight: `.rpg/base.json` and `.rpg/embeddings.bin` contain
**repo-relative paths** and are **portable across machines**.

```sh
# Alice encodes + enriches (pays the LLM cost once)
cd your-repo
RPG_SEMANTIC=true rpg-mcp &  # encode, then Ctrl+C after it finishes

# Alice commits the graph
git add .rpg/base.json .rpg/embeddings.bin
git commit -m "Add pre-computed RPG graph"

# Bob clones — zero LLM cost, instant graph
git clone your-repo
# Bob's rpg-mcp loads .rpg/base.json on startup, no encoding needed
```

The `.rpg/memories.jsonl` is also committable — team notes travel with the repo.
For personal memories, set `RPG_MEMORY_FILE=~/.rpg/my-memories.jsonl`.

> **What to commit:** `.rpg/base.json`, `.rpg/embeddings.bin`, `.rpg/memories.jsonl`
> **What NOT to commit:** `.rpg/dir_hash` (machine-specific), `.rpg/patches/` (transient)

---

## Environment variable reference

### Required

| Variable | Description |
|----------|-------------|
| `RPG_WORKSPACE` | **Required.** Absolute path to the repository root. |

### Server configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `RPG_DATA_DIR` | `<workspace>/.rpg` | Data directory for graph store, embeddings, memories. |
| `RPG_HASH_MODE` | `mtime` | Change detection: `mtime` (fast) or `content` (accurate). |
| `RPG_SEMANTIC` | `false` | Set to `true`/`1` to enable LLM semantic enrichment on encode. |
| `RPG_ENV_FILE` | _(auto-discovered)_ | Explicit `.env` file path. Otherwise searches workspace, cwd, etc. |
| `RPG_MEMORY_FILE` | `<workspace>/.rpg/memories.jsonl` | Agent memory store location. |
| `RPG_TELEMETRY_FILE` | _(disabled)_ | If set, writes JSONL telemetry per tool call. |
| `RUST_LOG` | `info` | Tracing filter (e.g. `rpg_mcp=debug`, `warn`). Logs go to stderr. |

### LLM configuration (required if `RPG_SEMANTIC=true`)

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENAI_API_KEY` | — | API key for the LLM endpoint. |
| `OPENAI_BASE_URL` | `https://api.openai.com/v1` | Base URL (any OpenAI-compatible API). |
| `OPENAI_MODEL` | `gpt-4o-mini` | Model name. |
| `OPENAI_MAX_CONCURRENT` | `3` | Max concurrent LLM HTTP requests. |
| `OPENAI_REASONING` | `false` | Enable reasoning mode (some models). |
| `RPG_DEBUG` | `false` | Dump full LLM request/response to stderr. |
| `RPG_DEBUG_FILE` | — | Write LLM request/response JSON to this file. |

### Embedding configuration (optional)

| Variable | Default | Description |
|----------|---------|-------------|
| `RPGEN_EMBEDDING_ENDPOINT` | `http://localhost:8994/v1` | Embedding API endpoint. |
| `RPGEN_EMBEDDING_MODEL` | `Qwen3-Embedding-8B-f16.gguf` | Embedding model name. |
| `RPGEN_EMBEDDING_DIMENSION` | `4096` | Vector dimension the model produces. |
| `RPGEN_EMBEDDING_BATCH_SIZE` | `64` | Texts per embedding request. |
| `RPGEN_EMBEDDING_MAX_CONCURRENT` | `4` | Max concurrent embedding HTTP requests (independent from LLM). |

---

## Tool reference (30 tools)

### Getting started

| Tool | Description |
|------|-------------|
| `encode_repo` | Full re-encode of the workspace repository |
| `detect_changes` | Incrementally re-encode only changed files (faster than encode_repo) |
| `get_graph_summary` | Overview: node/edge counts, languages, validation report |

### Finding code

| Tool | Description |
|------|-------------|
| `search_nodes` | Find by name substring, with optional kind/category filters |
| `get_node_details` | Full info for a node (detail_level: minimal/summary/full) |
| `semantic_search` | Find by WHAT CODE DOES (keyword search over LLM-extracted features) |
| `vector_search` | Embedding similarity search ("find code that does X") |
| `find_node_at_location` | Given file+line, find the enclosing definition |

### Navigating relationships

| Tool | Description |
|------|-------------|
| `get_callers` | Who calls X (transitive with depth>1) |
| `get_callees` | What does X call (transitive with depth>1) |
| `explore_graph` | BFS traversal in any direction, any depth, edge-type filtered |
| `get_edges` | Direct edge query by source/target/type |
| `get_impact` | Blast radius of changing a node (callers + callees + affected files) |
| `get_source` | Read the actual source lines for a node |
| `find_dead_code` | Definitions with no incoming calls/references (removal candidates) |

### Understanding architecture

| Tool | Description |
|------|-------------|
| `get_feature_tree` | The functional hierarchy (behavioral areas + members) |
| `get_features` | Nodes carrying LLM-extracted features |
| `get_components` | Logical component groupings |
| `get_skeleton` | File → children structure |
| `get_architecture_overview` | Hub nodes, centroid distribution, language breakdown |

### Cross-language / FFI

| Tool | Description |
|------|-------------|
| `get_ffi_bindings` | What native functions a language binds to (unique to rpg-mcp) |

### Change analysis

| Tool | Description |
|------|-------------|
| `analyze_diff` | Graph-aware diff analysis with risk scoring |
| `get_changed_nodes` | Which nodes changed since a git ref |

### Code intelligence

| Tool | Description |
|------|-------------|
| `encode_embeddings` | Compute vector embeddings over graph nodes |
| `enrich_with_scip` | Refine edges with compiler-grade SCIP symbol resolution |

### Agent memory

| Tool | Description |
|------|-------------|
| `write_memory` | Write a cross-session memory note |
| `read_memory` | Read a memory by id |
| `list_memories` | List memories with optional filters |
| `delete_memory` | Delete a memory by id |

### Export

| Tool | Description |
|------|-------------|
| `export_graph` | Export to GraphML / Cypher / DOT for external tools |

---

## Supported languages (14)

Rust, Python, Go, C, C++, JavaScript, TypeScript, Java, Ruby, Lua, Swift,
Haskell, C#, Scala.

---

## Docker deployment

```sh
# Build
docker build -t rpg-mcp:latest ./rpg-mcp

# Run (mount your repo, set workspace)
docker run --rm -i \
  --volume /path/to/repo:/repo \
  --env RPG_WORKSPACE=/repo \
  --env RPG_SEMANTIC=true \
  --env OPENAI_API_KEY=your-key \
  rpg-mcp:latest
```

The Dockerfile uses a multi-stage build (`rust:1.82` builder → `debian:bookworm-slim`
runtime). The final image is ~50MB.

---

## Troubleshooting

### "RPG_WORKSPACE env var is required"

The server needs to know which directory to analyze. Set `RPG_WORKSPACE` to
your repo's absolute path in the `env` block of your client config.

### Server starts but tools return empty results

The initial encode may still be running (it parses every file). Check stderr
logs (`RUST_LOG=info`) for the "Semantic encoding complete" or "encode complete"
message. For large repos, the first encode can take 30+ seconds.

### Semantic search returns nothing

You need `RPG_SEMANTIC=true` **at encode time** (not just at query time). If the
graph was encoded without semantic enrichment, re-encode:
1. Delete `.rpg/` (or just `.rpg/base.json`)
2. Set `RPG_SEMANTIC=true` + LLM env vars
3. Restart the server

### vector_search returns "unavailable"

You need to call `encode_embeddings` first (after a semantic encode). The
embedding endpoint must be reachable. Check `RPGEN_EMBEDDING_ENDPOINT`.

### Logs are noisy / interfering with the client

All logs go to **stderr** — stdout is reserved for the MCP protocol. If logs
are too verbose, set `RUST_LOG=warn`. Never set `RUST_LOG=debug` in production
(it dumps LLM payloads).

### The file watcher isn't detecting changes

The watcher skips `.rpg/`, `.git/`, `target/`, `node_modules/`, `dist/`, `build/`.
If your code lives under a skipped directory, it won't be watched. Also, the
watcher has a 2-second debounce — very rapid saves may coalesce.

### Portability: graph works on my machine but not my teammate's

Node paths are repo-relative, so `.rpg/base.json` is portable. The most common
issue is a stale `.rpg/dir_hash` — delete it and let the server recompute. Also
ensure `RPG_WORKSPACE` points to the same relative position within the repo.

---

## Architecture overview

```
┌─────────────────────────────────────────────────────┐
│                   MCP Client                         │
│  (Claude Desktop, Cursor, opencode, ZCode, etc.)    │
└──────────────────────┬──────────────────────────────┘
                   stdio (JSON-RPC)
┌──────────────────────┴──────────────────────────────┐
│                   rpg-mcp server                     │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │  30 tools   │  │ File watcher │  │  Telemetry │ │
│  └──────┬──────┘  └──────┬───────┘  └────────────┘ │
│         │                 │                          │
│  ┌──────┴─────────────────┴──────────────────────┐ │
│  │              AppState (shared)                 │ │
│  │  RpgGraph  │  RpgSnapshot  │  Embeddings      │ │
│  │  Memories  │  RpgStore     │  ParserRegistry  │ │
│  └───────────────────────────────────────────────┘ │
└──────────────────────┬──────────────────────────────┘
                       │
┌──────────────────────┴──────────────────────────────┐
│              rpg-encoder (core library)              │
│  Tree-sitter parsers (14 languages)                  │
│  Graph builder + link passes                         │
│  LLM enrichment + functional abstraction             │
│  Embeddings (FlatIndex / zvec)                       │
│  Incremental evolution                               │
│  SCIP enrichment                                     │
└──────────────────────────────────────────────────────┘
```

## License

Apache-2.0
