# Architecture

This document describes the architecture of the RPG Encoder system.

## Overview

```
+------------------+     +------------------+     +------------------+
|   Source Files   | --> |  Tree-sitter     | --> |  ParseResult     |
|  (.rs, .py, ...) |     |  Parsers         |     |  (definitions,   |
+------------------+     +------------------+     |   calls, refs)   |
                                                  +--------+---------+
                                                           |
                                                           v
+------------------+     +------------------+     +--------+---------+
|    RpgGraph      | <-- |  GraphBuilder    | <-- |  LanguageParser  |
|  (nodes, edges)  |     |  (link nodes)    |     |  (per-language)  |
+--------+---------+     +------------------+     +------------------+
         |
         | [semantic feature]
         v
+--------+---------+     +------------------+     +------------------+
| SemanticEnricher | --> | EmbeddingClient  | --> | Node.embedding   |
| (add embeddings) |     | (mock/openai)    |     +------------------+
+--------+---------+     +------------------+
         |
         v
+--------+---------+     +------------------+
| EdgeDetector     | --> | Semantic Edges   |
| (similarity)     |     | (requires, etc)  |
+------------------+     +------------------+
```

## Core Components

### Node

A node represents a code entity:

```
+----------------------------------+
|              Node                |
+----------------------------------+
| id: NodeId                       |
| category: NodeCategory           |
| kind: String                     |  // tree-sitter node kind
| language: String                 |  // rust, python, go, ...
| name: String                     |  // identifier
| path: Option<PathBuf>            |  // source file
| location: Option<SourceLocation> |  // line/column
| signature: Option<String>        |  // function signature
| documentation: Option<String>    |  // extracted docs
| source_ref: Option<SourceRef>    |  // line range
| semantic: Option<SemanticData>   |  // [semantic feature]
+----------------------------------+
```

### Edge

Edges represent relationships:

```
+------------------+
|      Edge        |
+------------------+
| edge_type: EdgeType
| metadata: HashMap |
+------------------+

EdgeType:
  Contains      -- parent contains child
  Imports       -- import statement
  Calls         -- function call
  Extends       -- inheritance
  Implements    -- trait/interface impl
  References    -- symbol reference
  DependsOn     -- general dependency
  FfiBinding    -- FFI boundary
  [semantic]
  RequiresFeature  -- needs feature from target
  EnablesFeature   -- provides feature to target
  RelatedFeature   -- semantically similar
```

### RpgGraph

The main graph structure:

```
+----------------------------------+
|            RpgGraph              |
+----------------------------------+
| nodes: Vec<Node>                 |
| edges: Vec<(NodeId, NodeId, Edge)> |
+----------------------------------+
| add_node(Node) -> NodeId         |
| add_edge(src, tgt, Edge)         |
| get_node(NodeId) -> Option<&Node>|
| nodes() -> Iterator<&Node>       |
| edges() -> Iterator<(src,tgt,Edge)>|
+----------------------------------+
```

## Encoding Pipeline

### Phase 1: Discovery

```
+------------+     +---------------+     +----------------+
| Root Path  | --> | FileWalker    | --> | Filtered Files |
|            |     | + .rpgignore  |     | (by extension) |
+------------+     +---------------+     +----------------+
```

1. Walk directory tree
2. Apply `.rpgignore` patterns
3. Filter by registered parser extensions

### Phase 2: Parsing

```
+----------------+     +---------------+     +----------------+
| Source File    | --> | Tree-sitter   | --> | ParseResult    |
|                |     | Parser        |     | - definitions  |
+----------------+     +---------------+     | - calls        |
                                             | - imports      |
                                             | - type_refs    |
                                             +----------------+
```

Each language has a `LanguageParser` implementation that:
- Parses source with tree-sitter
- Extracts definitions (functions, types, etc.)
- Extracts calls and references
- Identifies FFI boundaries

### Phase 3: Graph Building

```
+----------------+     +---------------+     +----------------+
| ParseResult    | --> | GraphBuilder  | --> | RpgGraph       |
| (per file)     |     | - create nodes|     | (partial)      |
+----------------+     | - add to graph|     +----------------+
                       +---------------+
                              |
                              v
                       +---------------+
                       | link_all()    |
                       | - resolve refs|
                       | - create edges|
                       +---------------+
```

The builder:
1. Creates nodes for each definition
2. Stores references for later resolution
3. After all files processed, resolves references to NodeIds
4. Creates edges for calls, imports, etc.

## Semantic Enrichment

### Phase 4: Embedding (Optional)

```
+----------------+     +---------------+     +----------------+
| RpgGraph       | --> | TextBuilder   | --> | Embedding Text |
| (parsed nodes) |     | (strategy)    |     | per node       |
+----------------+     +---------------+     +----------------+
                              |
                              v
                       +---------------+
                       | EmbeddingClient|
                       | (async embed) |
                       +---------------+
                              |
                              v
                       +----------------+
                       | Node.semantic  |
                       | .embedding     |
                       +----------------+
```

Embedding strategies:
- **Code**: Source code + signature + docs
- **Semantic**: Feature name + description + patterns
- **Hybrid**: Weighted combination (default)
- **MultiVector**: Separate code and semantic vectors

### Phase 5: Edge Detection (Optional)

```
+----------------+     +---------------+     +----------------+
| Nodes with     | --> | Cosine        | --> | Similar Pairs  |
| embeddings     |     | Similarity    |     | (threshold)    |
+----------------+     +---------------+     +----------------+
                                                     |
                                                     v
                                              +---------------+
                                              | Edge Type     |
                                              | by role       |
                                              +---------------+
```

Edge type determination:
- Provider + Provider → `RelatedFeature`
- Provider + Consumer → `EnablesFeature` / `RequiresFeature`
- Consumer + Consumer → `RelatedFeature`

### Phase 6: Clustering (Optional)

```
+----------------+     +---------------+     +----------------+
| Similar Nodes  | --> | Cluster       | --> | FunctionalCluster|
| (edges)        |     | Detection     |     | - name         |
+----------------+     +---------------+     | - members[]    |
                                             +----------------+
```

Clusters are named by:
1. Common prefix of member names (`auth_*` → `auth`)
2. Or shared domain (`logic`, `data`, etc.)

## Data Flow Summary

```
Source Files
     |
     v
Tree-sitter Parse
     |
     v
ParseResult (per file)
     |
     v
GraphBuilder
     |
     v
RpgGraph (structural)
     |
     +--[semantic]--+
     |              |
     v              v
TextBuilder   SourceCache
     |              |
     +------+-------+
            |
            v
     EmbeddingClient
            |
            v
RpgGraph (enriched)
            |
            v
     EdgeDetector
            |
            v
RpgGraph (+ semantic edges)
            |
            v
     ClusterFinder
            |
            v
  FunctionalCluster[]
```

## File Structure

```
rpg-encoder/src/
├── lib.rs              # Public exports
├── core/
│   ├── node.rs         # Node, NodeCategory, SemanticData
│   ├── edge.rs         # Edge, EdgeType
│   ├── graph.rs        # RpgGraph
│   ├── id.rs           # NodeId, EdgeId
│   └── location.rs     # SourceLocation
├── encoder/
│   ├── mod.rs          # RpgEncoder, EncodeResult
│   ├── builder.rs      # GraphBuilder
│   ├── walker.rs       # FileWalker
│   ├── output.rs       # JSON serialization
│   ├── semantic.rs     # SemanticEnricher, SimilaritySearch
│   └── semantic_edges.rs # SemanticEdgeDetector
├── embedding/
│   ├── mod.rs          # Exports
│   ├── config.rs       # EmbeddingConfig, strategies
│   ├── client.rs       # EmbeddingClient trait
│   ├── openai_compat.rs # OpenAI-compatible client
│   ├── mock.rs         # MockEmbeddingClient
│   └── text_builder.rs # EmbeddingTextBuilder
├── parser/
│   ├── mod.rs          # ParserRegistry
│   ├── trait.rs        # LanguageParser trait
│   ├── docs.rs         # Documentation extraction
│   └── base.rs         # TreeSitterParser
├── languages/
│   ├── mod.rs          # Language module exports
│   ├── rust.rs         # Rust parser
│   ├── ffi.rs          # FFI detection
│   └── ...             # Other language parsers
├── config/
│   ├── mod.rs          # RpgConfig
│   └── loader.rs       # ConfigLoader (YAML + env)
├── error.rs            # Error types
└── llm/                # [llm feature]
    └── agents/         # Feature extraction
```
