# RPG Chunked JSON Storage with Global Patch Layer

## Summary

A storage backend for Repository Planning Graphs that stores a full graph as a JSON base file and records incremental changes as append-only global patch files. Each patch captures all file changes in a single update cycle. Periodic compaction merges patches back into the base. The design prioritizes simplicity, debuggability, and cross-language portability (Rust + Python RPG-ZeroRepo interop).

## Requirements

- **Compact**: JSON with optional zstd compression; no redundant per-field deltas
- **Fast load**: Single deserialize of base + sequential patch application (~10-40ms for 10K nodes)
- **Patchable**: Per-file granularity patches within a global patch file; append-only, no in-place mutation of existing files
- **Portable**: Standard JSON readable by any language; patches are self-describing
- **Evolvable**: Base format is the existing `SerializedGraph`; patches extend it naturally

## File Layout

```
.rpg/
├── manifest.json          # Metadata: version, patch list, timestamps, file hashes
├── base.json              # Full RPG snapshot (compact JSON, no pretty-print)
├── patches/
│   ├── 001.json
│   ├── 002.json
│   └── ...
└── base.json.zst          # Optional: zstd-compressed base (replaces base.json)
```

The `.rpg/` directory lives at the repository root, alongside `.rpgignore`.

## Manifest

```json
{
  "version": 1,
  "repo_name": "my-project",
  "repo_info": "A description of the repository",
  "base": {
    "timestamp": 1713014400,
    "node_count": 8742,
    "edge_count": 15430,
    "file_hash": "sha256:a1b2c3...",
    "compressed": false
  },
  "patches": [
    { "seq": 1, "timestamp": 1713015000, "files": 3, "size_bytes": 12340 },
    { "seq": 2, "timestamp": 1713016000, "files": 1, "size_bytes": 4521 }
  ],
  "compaction_threshold": {
    "max_patches": 10,
    "max_size_ratio": 0.5
  },
  "file_index": {
    "src/main.rs": { "hash": "sha256:abc...", "base_node_ids": ["node_0"] },
    "src/parser.rs": { "hash": "sha256:def...", "base_node_ids": ["node_10", "node_11"] }
  }
}
```

### Fields

| Field | Purpose |
|-------|---------|
| `version` | Format version for forward compatibility |
| `base.file_hash` | SHA-256 of `base.json` for integrity verification |
| `base.compressed` | If true, `base.json.zst` exists instead of `base.json` |
| `patches[].seq` | Monotonically increasing sequence number |
| `compaction_threshold` | When `len(patches) > max_patches` or total patch size > `max_size_ratio * base_size`, trigger compaction |
| `file_index` | Maps file paths to their SHA-256 content hash and node IDs in the base. Used by the diff engine to detect which files changed without re-scanning the graph |

The `file_index` is rebuilt during compaction and on initial save. It enables the diff engine to compare current file hashes against stored hashes directly, without loading the full graph.

## Base File

The base file is a JSON document combining the current `SerializedGraph` format with snapshot metadata:

```json
{
  "nodes": [ { ... }, ... ],
  "edges": [ { ... }, ... ],
  "metadata": {
    "version": "0.1.0",
    "languages": ["rust", "python"],
    "node_count": 8742,
    "edge_count": 15430
  },
  "file_hashes": {
    "src/main.rs": "sha256:abc...",
    "src/lib.rs": "sha256:def..."
  },
  "unit_cache": {
    "src/main.rs": [
      {
        "name": "main",
        "unit_type": "Function",
        "content_hash": "sha256:ghi...",
        "start_line": 1,
        "end_line": 5,
        "features": [],
        "description": "",
        "node_id": "node_5"
      }
    ]
  }
}
```

This reuses the existing `SerializedNode` and `SerializedEdge` types from `encoder/output.rs`. The `file_hashes` and `unit_cache` fields come from `RpgSnapshot` and are included for incremental diff computation.

## Patch File

Each patch is a single JSON file recording all changes from one update cycle:

```json
{
  "seq": 1,
  "timestamp": 1713015000,
  "parent_seq": 0,
  "changes": {
    "added_files": ["src/new_module.rs"],
    "deleted_files": ["src/deprecated.rs"],
    "modified_files": {
      "src/parser.rs": {
        "old_hash": "sha256:aaa...",
        "new_hash": "sha256:bbb...",
        "removed_node_ids": ["node_42", "node_43"],
        "added_nodes": [
          {
            "id": "node_9001",
            "category": "function",
            "kind": "function",
            "language": "rust",
            "name": "parse_expression",
            "path": "src/parser.rs",
            "location": {
              "file": "src/parser.rs",
              "start_line": 45,
              "start_column": 1,
              "end_line": 80,
              "end_column": 2
            }
          }
        ],
        "added_edges": [
          { "source": "node_10", "target": "node_9001", "type": "calls" }
        ],
        "removed_edges": [
          { "source": "node_10", "target": "node_42", "type": "calls" }
        ]
      }
    }
  },
  "stats": {
    "files_added": 1,
    "files_deleted": 1,
    "files_modified": 1,
    "nodes_added": 5,
    "nodes_removed": 2,
    "edges_added": 3,
    "edges_removed": 1
  }
}
```

### Patch semantics

- **Modified nodes**: Represented as removal of the old node + addition of the new node. No partial field updates. This avoids merge conflicts and keeps the patch format simple.
- **`parent_seq`**: Chains patches in order. Enables detection of corrupted or out-of-order patches on load.
- **Node IDs in patches**: Use the same `"node_N"` string format as `SerializedGraph`. New nodes get IDs that do not collide with existing base or prior patch IDs (the `RpgGraph` auto-increments, and the patch serialization captures the assigned ID).
- **Unit cache updates**: Implicit. When a file's nodes are removed and re-added, the unit cache for that file is rebuilt during patch application. Patches do not carry unit cache entries — the unit cache is a derived artifact rebuilt from node data.

## Load Pipeline

```
.rpg/manifest.json
       │
       ▼
  read manifest
       │
       ▼
  read base.json (or decompress base.json.zst)
       │
       ▼
  serde_json::deserialize → RpgSnapshot
       │
       │  for each patch in manifest.patches (ordered by seq)
       │       │
       │       ▼
       │  read patches/{seq}.json
       │       │
       │       ▼
       │  apply_patch(snapshot, patch)
       │
       ▼
  rebuild_node_id_index()
  build_reverse_deps()
       │
       ▼
  RpgSnapshot ready
```

### `apply_patch` algorithm

1. **Deleted files**: For each path in `deleted_files`, call `snapshot.graph.remove_file_nodes(path)`. Remove from `file_hashes` and `unit_cache`.
2. **Added files**: For each path in `added_files`, create a `File` node and insert into graph. Add to `file_hashes`.
3. **Modified files**: For each entry in `modified_files`:
   a. Remove nodes by `removed_node_ids` — call `snapshot.graph.remove_node(id)` for each.
   b. Add nodes from `added_nodes` — deserialize into `Node` and call `snapshot.graph.add_node()`.
   c. Remove edges from `removed_edges` — match by `(source, target, type)` and call `snapshot.graph.remove_edge_between()`.
   d. Add edges from `added_edges` — call `snapshot.graph.add_edge()`.
   e. Update `snapshot.file_hashes[path]` to `new_hash`.
4. **Post-application**: `rebuild_node_id_index()`, `build_reverse_deps()`.

### Performance estimate (10K nodes, no embeddings)

| Scenario | Estimated time |
|----------|---------------|
| Load base only (0 patches) | 10-20ms |
| Load base + 5 patches | 20-40ms |
| Load base + 10 patches (pre-compaction) | 30-60ms |
| Load compacted base | 10-20ms |

## Write Pipeline

```
  Source files changed
        │
        ▼
  generate_diff()            ← already exists in incremental/diff.rs
  → FileDiff
        │
        ▼
  RpgEvolution::process_diff()   ← already exists in incremental/evolution.rs
  → mutates RpgSnapshot in-memory
  → returns EvolutionSummary
        │
        ▼
  NEW: capture_patch()      ← diff before/after in-memory state
  → Patch struct             ← extract removed/added node IDs and edges per file
        │
        ▼
  RpgStore::write_patch()   ← serialize patch → patches/{seq}.json
  → update manifest.json     ← atomic: write tmp, rename
        │
        ▼
  RpgStore::should_compact() ← check thresholds
  → if yes: compact()
```

### Patch capture strategy

The `RpgEvolution::process_diff()` method already mutates the in-memory `RpgSnapshot`. To capture the patch:

1. **Before evolution**: snapshot the set of node IDs and edge tuples per file from the in-memory graph.
2. **After evolution**: diff the new in-memory graph against the snapshot.
3. **For each changed file**: collect `removed_node_ids` (present before, absent after), `added_nodes` (absent before, present after), `removed_edges`, `added_edges`.

This avoids recomputing the file diff. The evolution engine already knows which files changed; the patch capture just records the graph-level consequences.

## Compaction

When compaction threshold is exceeded:

1. Load base + apply all patches → fully merged `RpgSnapshot`.
2. Serialize as new `base.json` (compact, no pretty-print).
3. Write to `.rpg/tmp/base.json`, verify, rename to `.rpg/base.json` (atomic).
4. Delete all files in `patches/`.
5. Rebuild `manifest.json` with new base metadata, empty patches list, updated `file_index`.
6. Keep old base as `.rpg/base.json.bak` until new base is verified.
7. Optionally compress: `base.json` → `base.json.zst`, delete uncompressed.

### Default thresholds

| Parameter | Default | Meaning |
|-----------|---------|---------|
| `max_patches` | 10 | Maximum number of unapplied patches |
| `max_size_ratio` | 0.5 | Trigger when total patch size > 50% of base size |

## Optional zstd Compression

- After compaction (or on explicit request), compress `base.json` → `base.json.zst`.
- Manifest records `"compressed": true`.
- On load, detect and decompress before deserializing.
- Patches are always uncompressed (small, append-only).
- Expected compression ratio: ~60-70% reduction.
- Dependency: `zstd` crate (pure Rust, no C bindings).

## API Surface

New module: `rpg-encoder/src/storage/`

```rust
mod manifest;
mod patch;
mod store;

pub use manifest::{Manifest, BaseInfo, PatchInfo, FileEntry, CompactionThreshold};
pub use patch::{Patch, PatchChanges, FilePatch, RemovedEdge, PatchStats};
pub use store::RpgStore;
```

### Core types

```rust
pub struct RpgStore {
    root: PathBuf,       // .rpg/ directory
    manifest: Manifest,
}

impl RpgStore {
    /// Initialize a new .rpg/ directory at the repo root.
    pub fn init(repo_path: &Path) -> Result<Self>;

    /// Open an existing .rpg/ directory.
    pub fn open(repo_path: &Path) -> Result<Self>;

    /// Load the full RPG: base + all patches.
    pub fn load(&self) -> Result<RpgSnapshot>;

    /// Save a snapshot as the base (overwrites existing base).
    pub fn save_base(&mut self, snapshot: &RpgSnapshot) -> Result<()>;

    /// Write a new patch. Updates manifest. Checks compaction threshold.
    pub fn write_patch(&mut self, patch: Patch) -> Result<()>;

    /// Compact: merge all patches into base.
    pub fn compact(&mut self) -> Result<()>;

    /// Whether compaction threshold is exceeded.
    pub fn should_compact(&self) -> bool;

    /// Number of unapplied patches.
    pub fn patch_count(&self) -> usize;

    /// Get the manifest (read-only).
    pub fn manifest(&self) -> &Manifest;

    /// Capture a patch by diffing before/after snapshots.
    pub fn capture_patch(
        &self,
        before: &RpgSnapshot,
        after: &RpgSnapshot,
        file_diff: &FileDiff,
    ) -> Patch;
}
```

## Integration with Existing Code

| Module | Change |
|--------|--------|
| `encoder/output.rs` | No changes. Kept for single-file JSON export (LLM context injection). |
| `incremental/snapshot.rs` | `save()` and `load()` gain `RpgStore` backend. Existing direct JSON methods remain for backward compat. |
| `incremental/evolution.rs` | `process_diff()` returns enough info to construct a `Patch`. Caller (the encoder pipeline) calls `RpgStore::write_patch()`. |
| `incremental/diff.rs` | No changes. Already produces `FileDiff`. |
| `encoder/mod.rs` | `RpgEncoder` gains `store()` method returning `&RpgStore`. Encode pipeline calls `store.save_base()` on first encode, `store.write_patch()` on incremental updates. |
| `Cargo.toml` | Add optional `zstd` dependency. |

## Error Handling and Corruption Safety

| Scenario | Handling |
|----------|----------|
| Crash during patch write | Patch written to `tmp/patch.json` first, then renamed. If crash before rename, manifest still points to old state. |
| Crash during manifest update | Patches are written before manifest. Extra patch files without manifest entry are ignored. |
| Corrupt patch file | On load, detect parse error. Skip patch and warn. Subsequent patches that depend on it (via `parent_seq`) will also fail validation. |
| Wrong `parent_seq` chain | Stop applying patches at the break. Log warning. Return partially-loaded snapshot. |
| Corrupt base file | Verify against `manifest.base.file_hash`. If mismatch, look for `base.json.bak`. |
| Missing `.rpg/` directory | `RpgStore::open()` returns error. Caller falls back to fresh encode. |

### Atomic writes

All file writes use the write-to-temp-then-rename pattern:

```rust
fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
```

## Testing Strategy

| Test type | Scope |
|-----------|-------|
| Unit tests | Patch serialize/deserialize roundtrip; patch application (add/remove/modify nodes and edges); manifest parsing |
| Property tests | `proptest`: random graph mutations → save → patch → load → verify graph matches in-memory state |
| Integration tests | Full pipeline: encode → save_base → modify files → diff → evolve → write_patch → load → verify |
| Corruption tests | Truncated patch, missing manifest, wrong `parent_seq`, corrupt base hash |
| Compaction tests | Write N patches → compact → load → verify equals in-memory graph after all patches applied |
| Zstd tests | Compressed base roundtrip; load detects compressed flag correctly |

## Performance Summary (10K nodes, no embeddings)

| Operation | Time | Disk size |
|-----------|------|-----------|
| Fresh encode + save_base | 50-100ms | 1.5-3 MB |
| Load (base only) | 10-20ms | — |
| Load (base + 5 patches) | 20-40ms | +25-250 KB patches |
| Load (base + 10 patches, pre-compaction) | 30-60ms | +50-500 KB patches |
| Load (compacted base) | 10-20ms | 1.5-3 MB |
| Write single patch | 2-5ms | 5-50 KB |
| Compaction | 30-60ms | Replaces base |
| Load (zstd-compressed base) | 15-30ms | 0.5-1 MB |

## Future Extensions (out of scope)

- **Partitioned base**: Split `base.json` into per-file JSON files for spot reads without full load. The `file_index` in the manifest already maps files to node IDs, enabling this transition.
- **Per-file JSONL patches**: Replace global patches with per-file append-only JSONL files for finer-grained spot reads. The `apply_patch` logic would remain the same; only the file layout changes.
- **Embedding sidecar**: Store vector embeddings in separate `.rpg/embeddings/` files with their own patch mechanism, keeping the structural graph small.
- **Python interop**: The JSON formats are designed to be parseable by the upstream Python RPG-ZeroRepo. A future `rpg_patching.py` module could read and apply patches produced by Rust, and vice versa.
