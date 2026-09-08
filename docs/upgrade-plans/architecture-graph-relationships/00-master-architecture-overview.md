# Architecture Graph & GraphRAG: Master Architecture Overview

## 1. Executive Summary
This document provides the architectural blueprint for resolving the disconnected node issue (`EDGES: 0`, `CLUSTERS: 0`) and transforming Agent-Guidance-Rust into an enterprise-grade Code Architecture Knowledge Graph.

The system connects four core layers:
1. **AST & Incremental Indexer**: Parses AST syntax trees, resolves caller/callee symbols, and builds true inter-symbol edges.
2. **GraphRAG Community Engine**: Runs Leiden hierarchical community clustering and computes dependency coupling & blast radius.
3. **MCP Tooling & REST Surface**: Exposes graph queries (`callers`, `callees`, `blast_radius`, `references`) to LLMs and provides `/api/graph` to clients.
4. **Interactive Dashboard HUD**: Renders a force-directed canvas with curved bezier links, directional arrows, glowing hubs, and deep symbol inspection.

---

## 2. Root Cause Analysis

### 2.1 Mismatched Symbol IDs in Edges
- **Symbols Table**: Stores symbols with identifier format `{rel_path}::{kind}::{name}::L{line}` (e.g. `src/main.rs::function::run::L10`).
- **Legacy Edge Extractor**: `extract_edges_from_content` generated `source_id` as `{rel_path}::L{line}` (e.g. `src/main.rs::L15`).
- **Impact**: `symbol_edges` could never join with `symbols`. Foreign Key cascades failed, and graph nodes in frontend canvas had 0 matching edges.

### 2.2 Lack of Cross-File Symbol Resolution
- During incremental indexing, files were processed in isolation.
- Call statements like `AstEngine::extract_calls(...)` were only checked against symbols in the local file.
- Cross-module calls and re-exports were completely omitted.

### 2.3 Communities Format Serialization Desync
- `GraphRagEngine` persists `communities.json` as an object:
  ```json
  {
    "project_name": "Agent-Guidance-Rust",
    "detected_architecture": "Clean_Architecture",
    "communities": [ ... ]
  }
  ```
- Dashboard API returned this object directly as `data.communities`.
- Frontend checked `Array.isArray(data.communities)`, which evaluated to `false`, rendering `CLUSTERS: 0`.

### 2.4 Canvas Visualizer Layout & Edge Omission
- Canvas capped rendering to the first 75 symbols arbitrarily.
- Straight thin lines without arrows failed to convey call directions or distinction between imports and invocations.
- No visual feedback when inspecting nodes or viewing blast radius.

---

## 3. Target End-to-End Data Flow

```
[ Source Files (*.rs, *.js, *.py, *.go, *.kt) ]
                    │
                    ▼
       AstEngine (tree-sitter / syn)
                    │
         Extracted AST Calls & Symbols
                    │
                    ▼
     [ Symbol Resolver (resolver.rs) ]
       - Enclosing caller scope mapping
       - Import & namespace resolution
       - Cross-file symbol resolution pass
                    │
                    ▼
       SQLite: .agent-context/code_graph.db
       ┌──────────────────────────────┐
       │ symbols: (id, name, kind, ..)│
       │ symbol_edges: (src, dst, ..) │
       │ symbol_vectors & chunks      │
       └──────────────────────────────┘
                    │
       ┌────────────┴────────────┐
       ▼                         ▼
 [ GraphRAG Leiden Engine ]   [ MCP Tools & REST API ]
 - Hierarchical communities   - project_context:
 - Blast radius traversal       * callers / callees
 - Coupling metrics             * blast_radius
 - communities.json             * references
       │                         - GET /api/graph
       └────────────┬────────────┘
                    ▼
       [ Dashboard Obsidian HUD ]
       - Clustered Force-Directed Canvas
       - Curved Bezier directional links
       - HUD: SYMBOLS, EDGES, CLUSTERS
       - Symbol Inspector & Impact Radius
```

---

## 4. Database Schema Specifications

### `symbols` Table
```sql
CREATE TABLE symbols (
    id TEXT PRIMARY KEY,               -- e.g. "src/lib.rs::function::init::L24"
    name TEXT NOT NULL,                -- e.g. "init"
    kind TEXT NOT NULL,                -- "function" | "struct" | "enum" | "trait" | "impl"
    file_path TEXT NOT NULL,           -- "src/lib.rs"
    parent TEXT,                       -- Optional enclosing struct/module
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    signature TEXT,
    FOREIGN KEY (file_path) REFERENCES files(path) ON DELETE CASCADE
);
```

### `symbol_edges` Table
```sql
CREATE TABLE symbol_edges (
    source_id TEXT NOT NULL,           -- Caller / importer symbol ID
    target_id TEXT NOT NULL,           -- Callee / imported symbol ID
    edge_type TEXT NOT NULL,           -- "calls" | "imports" | "implements"
    weight REAL DEFAULT 1.0,           -- Call frequency / coupling weight
    FOREIGN KEY (source_id) REFERENCES symbols(id) ON DELETE CASCADE,
    FOREIGN KEY (target_id) REFERENCES symbols(id) ON DELETE CASCADE,
    PRIMARY KEY (source_id, target_id, edge_type)
);
```

---

## 5. Phase Breakdown Roadmap
- **Phase 1**: Symbol Resolution & AST Edge Extraction (`01-phase-symbol-resolution-indexer.md`)
- **Phase 2**: GraphRAG Community Engine & Blast Radius (`02-phase-graphrag-and-communities.md`)
- **Phase 3**: MCP Tooling & Backend APIs (`03-phase-mcp-tools-and-apis.md`)
- **Phase 4**: Dashboard HUD Canvas Visualizer (`04-phase-frontend-visualization.md`)
