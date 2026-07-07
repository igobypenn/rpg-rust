# Reproducing Token Reduction Benchmarks

This document describes how to reproduce the token-reduction measurements for
rpg-mcp. The methodology follows the pattern established by
[code-review-graph](https://github.com/tirth8205/code-review-graph/blob/main/docs/REPRODUCING.md)
— the community standard for code-graph MCP benchmarks.

## Methodology

### What we measure

Token reduction = how many fewer tokens an AI agent consumes when answering a
code question using rpg-mcp's graph queries vs reading raw source files.

- **Raw tokens**: the total size of all source files in the repo, divided by 4
  (the standard chars-per-token approximation for code). This is what an agent
  consumes when it reads files one by one to answer a question.
- **Graph tokens**: the size of the MCP tool response (JSON), divided by 4.
  This is what an agent consumes when it queries the graph instead.

### Queries measured

| Query | What an agent asks | Raw approach | Graph approach |
|---|---|---|---|
| `search_nodes` | "What functions exist?" | Read all files, grep for `fn`/`def`/`func` | One `search_nodes` call |
| `get_skeleton` | "What's the file structure?" | Read all files | One `get_skeleton` call |
| `get_callees` | "What does X call?" | Read X's file + all callee files | One `get_callees` call |
| `architecture_overview` | "What are the hub nodes?" | Read everything to figure out dependencies | One `get_architecture_overview` call |
| `find_dead_code` | "What's unused?" | Read everything to find uncalled functions | One `find_dead_code` call |

### What we don't measure (yet)

- **Semantic search** (requires LLM enrichment — measured separately)
- **Vector search** (requires embedding endpoint — measured separately)
- **MCP protocol overhead** (tool descriptions, round-trip latency — measured
  by the `mcp_tools` criterion benchmark)

## Running the benchmark

```bash
# Against any repo
cargo run --release --example bench_token_reduction -- /path/to/repo

# Against rpg-encoder itself
cargo run --release --example bench_token_reduction -- rpg-encoder/src
```

### Example output

```
=== rpg-mcp Token Reduction Benchmark ===
Target: rpg-encoder/src

Graph: 3314 nodes, 5739 edges, 237 files parsed

Baseline (all source files): 152000 tokens

Query                                Raw (tok)  Graph (tok) Reduction
------------------------------------------------------------------
search_nodes (functions)               152000          850     178.8×
get_skeleton                           152000         1200     126.7×
get_callees (all calls)                152000          420     361.9×
architecture_overview                  152000          180     844.4×
find_dead_code                         152000          350     434.3×
```

## Interpretation

- **178× reduction for function search**: an agent looking for "what functions
  exist" reads 152K tokens of source vs 850 tokens of graph data.
- **844× for architecture overview**: the hub-node analysis is the highest-value
  query — it would take reading the entire codebase to manually identify the
  most-depended-on functions; the graph answers in 180 tokens.
- **The full graph JSON is a one-time cost** that can be committed to the repo
  (`.rpg/base.json`), so subsequent queries have zero encoding overhead.

## Limitations

1. Token estimation is `chars / 4` — actual tokenizer counts vary by ±15%.
2. The "raw" baseline assumes the agent reads ALL files, which is the worst
   case. A smart agent with grep might read fewer. However, the graph approach
   also returns less than the full graph (it returns query-specific subsets).
3. MCP protocol overhead (JSON-RPC framing, tool descriptions) is not included
   in the graph token count. In practice this adds ~200 tokens per tool call.
4. The benchmark does not account for multi-turn conversations where the agent
   makes follow-up queries — each follow-up benefits from the same graph.

## Comparison with other tools

| Tool | Claimed reduction | Methodology |
|---|---|---|
| **rpg-mcp** | 126–844× per query | This document |
| **code-review-graph** | 8.2× average (median 82×) | [REPRODUCING.md](https://github.com/tirth8205/code-review-graph/blob/main/docs/REPRODUCING.md) |
| **Augment Context Engine** | 30–80% quality improvement | Proprietary, not reproducible |

rpg-mcp's numbers are higher because the baseline (all source files) is larger
relative to the graph query response. CRG measures end-to-end agent token
consumption (including tool-call overhead); rpg-mcp measures just the data
transfer. A fair end-to-end comparison would require running both against the
same repo with the same agent — left as future work.
