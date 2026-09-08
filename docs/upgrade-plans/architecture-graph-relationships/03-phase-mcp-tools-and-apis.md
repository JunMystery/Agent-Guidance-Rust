# Phase 3: MCP Tooling & Backend APIs

## 1. Objectives & Scope
Expand LLM tool capabilities with deep graph inspection operations and modernize the dashboard `/api/graph` endpoint.

Key deliverables:
1. `project_context` MCP Tool Operations:
   - `callers`: Retrieve all callers of a specified symbol or file.
   - `callees`: Retrieve all functions/methods called by a specified symbol.
   - `blast_radius`: Analyze modification impact, affected files, and risk severity.
   - `references`: Enhance with exact AST edge queries prior to FTS fallback.
2. Dashboard Endpoint `/api/graph`:
   - Return `{ nodes, edges, communities, stats }` with coherent foreign keys.
   - Degree-aware node selection: Ensure top connected nodes and their interconnecting edges are bundled together, eliminating phantom/orphaned edges.

---

## 2. Technical Design

### 2.1 MCP Operations Specification

#### `project_context(operation="callers", query="my_function")`
Response format:
```markdown
# Callers of 'my_function' (Count: 4)

- `src/handlers/api.rs::L45` -> `handle_request()` [calls, weight: 1.0]
- `src/services/auth.rs::L88` -> `verify_token()` [calls, weight: 1.0]
```

#### `project_context(operation="blast_radius", query="CodeGraphDb")`
Response format:
```markdown
# Blast Radius Analysis for 'CodeGraphDb'
- Direct Callers: 8 symbols across 4 files
- Transitive Impact: 22 symbols
- Risk Level: HIGH (Score: 8.2/10)
- Impacted Files:
  * `src/context/indexer/mod.rs`
  * `src/mcp/tools/context_graph.rs`
  * `src/dashboard/graph.rs`

```mermaid
graph TD
...
```
```

---

## 3. Implementation Blueprint (< 200 LOC per file)

### `src/mcp/tools/context_graph.rs` [MODIFIED]
- Add handlers:
  - `handle_callers(proj_path, query) -> String`
  - `handle_callees(proj_path, query) -> String`
  - `handle_blast_radius(proj_path, query) -> String`

### `src/dashboard/graph.rs` [MODIFIED]
- Query top connected symbols by degree (`COUNT(edges)`):
  ```sql
  SELECT s.id, s.name, s.kind, s.file_path, s.start_line, s.end_line,
         (SELECT COUNT(*) FROM symbol_edges e WHERE e.source_id = s.id OR e.target_id = s.id) as degree
  FROM symbols s
  ORDER BY degree DESC, (s.end_line - s.start_line) DESC
  LIMIT 250;
  ```
- Fetch only edges whose `source_id` AND `target_id` exist in the selected nodes.
- Expose `communities` as a direct array from `CommunityHierarchy.communities`.
