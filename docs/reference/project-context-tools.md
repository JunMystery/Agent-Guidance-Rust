# Project Context Tools

[Back to README](../README.md)

The `agent-guidance-mcp_project_context` tool provides a token-budgeted, persistent, and intelligent code exploration engine. It replaces slow raw-disk traversals with a local **SQLite + Tree-sitter + BERT Vector Search** cascade (<100ms) isolated per-project at `<project_root>/.agent-context/code_graph.db`.

---

## ⚡ 5-Phase Instant Search Cascade (<100ms)

When `operation="search"` is invoked, the engine processes queries through a 5-tier cascade:

```
Query: "xử lý timeout khi gọi API bên thứ 3"
  │
  ├── Phase 1: ALIAS CACHE (<1ms) ─────────── Direct lookup of learned natural language terms
  ├── Phase 2: SYMBOL FTS5 (<5ms) ─────────── Full-text match on function, class, struct names
  ├── Phase 3: SYMBOL VECTORS (<50ms) ─────── Multilingual-E5 semantic similarity on signatures
  ├── Phase 4: CONTENT FTS5 (<5ms) ────────── Full-text search across 50-line code chunks
  └── Phase 5: RAG CONTENT VECTORS (<100ms) ─ Semantic similarity on actual chunk logic
```

### Adaptive Alias Learning & Decay
- Successful searches and manual `learn_alias` calls are automatically saved with an initial confidence of `0.80`.
- Repeated hits increase confidence up to `1.0`.
- **Decay Policy**: Inactive aliases have their confidence halved after 30 days and are automatically purged after 90 days.

---

## 🛠️ Operations Reference

### 1. `operation="search"`
Executes the 5-phase search cascade across symbols, code content, and learned aliases.
```json
{
  "operation": "search",
  "project_path": "/path/to/project",
  "query": "process_payment"
}
```

### 2. `operation="navigate"`
Comprehensive code graph traversal returning all matching layers simultaneously (aliases, symbols, content chunks, vectors, and DAG call/import edges).
```json
{
  "operation": "navigate",
  "project_path": "/path/to/project",
  "query": "PaymentService",
  "scope": "all"
}
```
*`scope` options*: `"all"`, `"symbols"`, `"content"`, `"edges"`.

### 3. `operation="learn_alias"`
Explicitly maps a natural language phrase or query to a file and symbol.
```json
{
  "operation": "learn_alias",
  "project_path": "/path/to/project",
  "alias_term": "tính năng thanh toán",
  "relative_path": "src/payment/service.rs",
  "resolved_symbol": "PaymentService",
  "resolved_line": 42
}
```

### 4. `operation="reindex"`
Forces a full re-scan and AST re-indexing of the project code graph, spawning background threads for BERT vector embedding.
```json
{
  "operation": "reindex",
  "project_path": "/path/to/project"
}
```

### 5. `operation="read"`
Reads a bounded file range (enforcing the hard 300 LOC cap).
```json
{
  "operation": "read",
  "project_path": "/path/to/project",
  "relative_path": "src/main.rs"
}
```

### 6. `operation="symbols"` / `operation="structure"`
Extracts top-level function, struct, enum, and class declarations from a specific file.
```json
{
  "operation": "symbols",
  "project_path": "/path/to/project",
  "relative_path": "src/main.rs"
}
```

### 7. `operation="references"`
Finds all occurrences and usages of a target symbol across the repository.
```json
{
  "operation": "references",
  "project_path": "/path/to/project",
  "query": "PaymentService"
}
```

### 8. `operation="tree"`
Returns a top-level directory and file overview (capped at depth 2).
```json
{
  "operation": "tree",
  "project_path": "/path/to/project"
}
```

### 9. `operation="graph_rag"`
Executes Hierarchical Leiden GraphRAG across multi-level community hierarchies and DAG call/import relationships.
```json
{
  "operation": "graph_rag",
  "project_path": "/path/to/project",
  "query": "authentication flow",
  "mode": "drift"
}
```
*`mode` options*:
- `"global"`: Holistic reasoning leveraging Level 0 (Macro Subsystems) & Level 1 (Feature Modules) Community Summaries.
- `"local"`: Targeted entity search fanning out across 1-hop and 2-hop DAG call/import edges.
- `"drift"`: Dual-route search combining Top-down Community Context with Bottom-up factual AST traversal.
- `"basic"`: Standard top-$k$ fallback vector/FTS search.

---

## 🔄 Proactive Background File Watcher

The engine automatically runs an OS-level inotify/file watcher in the background:
- **5s Debounce**: File edits are buffered for 5 seconds before triggering incremental indexing and GraphRAG community re-clustering.
- **Incremental Indexing**: Uses SHA256 hashes to only re-parse modified files.
- **Safe Filtering**: Automatically excludes `.git`, `.agent-context`, `target`, `node_modules`, `build`, `__pycache__`, `dist`, `.next`, and binary files.

---

## 🔒 Per-Project Storage Isolation

All code graph databases, FTS5 virtual tables, and vector embeddings are stored locally within the project directory:
```
<project_root>/
└── .agent-context/
    ├── architecture.json      # Inferred architectural topology
    ├── communities.json       # GraphRAG Leiden Community Hierarchy & Summaries
    ├── code_graph.db          # SQLite DB (symbols, edges, vectors, chunks, aliases)
    └── sessions/              # Multi-session continuity state
```
No data is shared between different repositories.

---

## Related Docs

- [Usage Guide](../usage.md)
- [MCP Surface](mcp-surface.md)
- [Architecture Overview](../ARCHITECTURE.md)

